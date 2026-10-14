//! Scheduling: who is activated when, how time is counted, and the checks the
//! simulator makes against the global state that no agent can see.

use super::agent::activate;
use super::bits::Widths;
use super::memory::{AgentState, AsyncAgent, Command, Lead, Memory, Phase, Plan, Position, Target};
use super::world::{Action, Note, World};
use super::{HeadStep, ProbeEvent, RootedAsyncError, RootedAsyncResult, Schedule};
use crate::common::EdgeType;
use crate::p1tree::TreeEdge;
use ccm_core::{
    AgentId, AgentStatus, DeterministicRng, Metrics, NodeId, PortGraph, RoundKind, Termination,
};
use ccm_trace::{Checkpoint, Recorder, SimulationEvent};

pub(super) struct Run<'g, M, R> {
    world: World<'g>,
    metrics: M,
    recorder: R,
    probes: Vec<ProbeEvent>,
    steps: Vec<HeadStep>,
    constructed_at: Option<u64>,
    epoch: u64,
    activations: u64,
    /// Whether anything has changed since the current epoch began.
    changed: bool,
    widths: Widths,
    /// The most bits any agent's memory has held after a write.
    memory_bits: u32,
}

impl<'g, M: Metrics, R: Recorder> Run<'g, M, R> {
    pub fn new(graph: &'g PortGraph, root: NodeId, k: usize, metrics: M, recorder: R) -> Self {
        let k_u32 = u32::try_from(k).expect("agent count fits the dense ID domain");
        let mut agents: Vec<AsyncAgent> = (0..k_u32)
            .map(|index| AsyncAgent {
                position: Position::At(root),
                home: None,
                memory: Memory::unsettled(AgentId(index)),
            })
            .collect();
        // All agents start together, so the lowest ID knows it leads and
        // knows k.
        agents[0].memory.lead = Some(Lead {
            phase: Phase::Gather,
            seq: 0,
            command: Command::None,
            k: k_u32,
            group_size: k_u32,
            unsettled: k_u32,
            head_owner: None,
            prev_id: None,
            child_port: None,
            sibling_details: None,
            child_details: None,
            child_sibling: None,
            target: Target::Empty,
            skip_port: None,
            probe_limit: 0,
            batch: 0,
            batches_here: 0,
            scatter_mode: false,
            best: None,
            plan: Plan::Back,
            next_agent_id: None,
        });
        let mut occupants = vec![Vec::new(); graph.node_count()];
        occupants[root.index()] = (0..k_u32).map(AgentId).collect();
        let max_degree = (0..graph.node_count())
            .map(|index| {
                let node = NodeId(u32::try_from(index).expect("dense node IDs"));
                usize::from(graph.degree(node).unwrap_or(0))
            })
            .max()
            .unwrap_or(0);
        let widths = Widths::new(k, max_degree);
        let memory_bits = agents
            .iter()
            .map(|agent| widths.memory(&agent.memory))
            .max()
            .unwrap_or(0);
        Self {
            world: World {
                graph,
                agents,
                occupants,
                home_owner: vec![None; graph.node_count()],
            },
            metrics,
            recorder,
            probes: Vec::new(),
            steps: Vec::new(),
            constructed_at: None,
            epoch: 0,
            activations: 0,
            changed: false,
            widths,
            memory_bits,
        }
    }

    pub fn run(
        mut self,
        schedule: Schedule,
        epoch_limit: u64,
    ) -> Result<(RootedAsyncResult, M, R), RootedAsyncError> {
        self.checkpoint();
        let termination = match schedule {
            Schedule::Synchronous => self.run_synchronous(epoch_limit)?,
            Schedule::Random { seed } => self.run_asynchronous(epoch_limit, seed, None)?,
            Schedule::Starve {
                seed,
                agent,
                period,
            } => self.run_asynchronous(epoch_limit, seed, Some((agent, period.max(1))))?,
        };
        let tree = self.tree();
        Ok((
            RootedAsyncResult {
                termination,
                agents: self.world.agents,
                tree,
                epochs: self.epoch,
                activations: self.activations,
                probes: self.probes,
                steps: self.steps,
                constructed_at: self.constructed_at,
                memory_bits: self.memory_bits,
            },
            self.metrics,
            self.recorder,
        ))
    }

    /// Every agent completes one cycle per round, all reading the state the
    /// round began with: the paper's SYNC model, in which a round is an epoch.
    fn run_synchronous(&mut self, limit: u64) -> Result<Termination, RootedAsyncError> {
        let count = self.world.agents.len();
        loop {
            if self.finished() {
                return Ok(Termination::Completed);
            }
            if self.epoch >= limit {
                return Ok(Termination::RoundLimitReached { limit });
            }
            self.changed = false;
            for index in 0..count {
                self.changed |= self.world.land(agent(index));
            }
            let actions: Vec<Action> = (0..count)
                .map(|index| activate(&self.world, agent(index)))
                .collect();
            self.activations += count as u64;
            self.apply_round(actions)?;
            self.end_epoch()?;
        }
    }

    /// Writes from one round are applied together. An agent written by another
    /// while also changing its own memory, or written twice, would make the
    /// outcome depend on the order of application, which the model forbids.
    fn apply_round(&mut self, actions: Vec<Action>) -> Result<(), RootedAsyncError> {
        // Judged against the state the round began with, before any write
        // lands: an agent that changed nothing must not overwrite a write made
        // into it.
        let own_changed: Vec<bool> = actions
            .iter()
            .enumerate()
            .map(|(index, action)| action.own != *self.world.memory(agent(index)))
            .collect();
        let mut touched = own_changed.clone();
        for action in &actions {
            if let Some(other) = &action.other {
                let slot = &mut touched[other.id.index()];
                if *slot {
                    return Err(RootedAsyncError::WriteConflict(other.id));
                }
                *slot = true;
            }
        }
        for (index, action) in actions.into_iter().enumerate() {
            self.apply(agent(index), action, !own_changed[index])?;
        }
        Ok(())
    }

    /// One agent at a time, in an order the schedule picks: the paper's ASYNC
    /// model with atomic cycles. A move ends the cycle only on arrival, so a
    /// scout can be held on an edge, seen at neither end, for as long as the
    /// schedule leaves it there.
    fn run_asynchronous(
        &mut self,
        limit: u64,
        seed: u64,
        starved: Option<(AgentId, u32)>,
    ) -> Result<Termination, RootedAsyncError> {
        let count = self.world.agents.len();
        let mut rng = DeterministicRng::new(seed);
        let mut completed = vec![false; count];
        let mut remaining = count;
        let mut step = 0_u64;
        loop {
            if self.finished() {
                return Ok(Termination::Completed);
            }
            if self.epoch >= limit {
                return Ok(Termination::RoundLimitReached { limit });
            }
            step += 1;
            let id = match starved {
                Some((slow, period)) if step % u64::from(period) == 0 || count == 1 => slow,
                Some((slow, _)) => {
                    let pick = rng.index(count - 1).expect("count > 1");
                    agent(if pick >= slow.index() { pick + 1 } else { pick })
                }
                None => agent(rng.index(count).expect("at least one agent")),
            };
            self.activations += 1;
            let finished_cycle = if self.world.land(id) {
                self.changed = true;
                true
            } else {
                let action = activate(&self.world, id);
                let moving = action.depart.is_some();
                self.apply(id, action, false)?;
                !moving
            };
            if finished_cycle && !completed[id.index()] {
                completed[id.index()] = true;
                remaining -= 1;
            }
            if remaining == 0 {
                self.end_epoch()?;
                completed.fill(false);
                remaining = count;
                self.changed = false;
            }
        }
    }

    fn end_epoch(&mut self) -> Result<(), RootedAsyncError> {
        self.epoch += 1;
        self.metrics.logical_rounds(RoundKind::Other, 1);
        self.checkpoint();
        if !self.changed && !self.finished() {
            // Every agent acted on an unchanging state and none changed it:
            // nothing ever will.
            return Err(RootedAsyncError::Deadlock { epoch: self.epoch });
        }
        Ok(())
    }

    fn apply(
        &mut self,
        id: AgentId,
        action: Action,
        keep_own: bool,
    ) -> Result<(), RootedAsyncError> {
        let Position::At(node) = self.world.agents[id.index()].position else {
            unreachable!("only landed agents act");
        };
        if !keep_own {
            let before = self.world.agents[id.index()].memory.state;
            self.settle_check(id, before, action.own.state, node)?;
            if action.own != self.world.agents[id.index()].memory {
                self.changed = true;
            }
            self.world.agents[id.index()].memory = action.own;
            self.measure(id);
        }
        if let Some(other) = action.other {
            let target = other.id;
            if self.world.agents[target.index()].position != Position::At(node) {
                return Err(RootedAsyncError::Protocol("wrote an agent at another node"));
            }
            let before = self.world.agents[target.index()].memory.state;
            self.settle_check(target, before, other.state, node)?;
            self.world.agents[target.index()].memory = other;
            self.measure(target);
            self.changed = true;
        }
        if let Some(port) = action.depart {
            let edge = self
                .world
                .graph
                .traverse(node, port)
                .ok_or(RootedAsyncError::Protocol(
                    "left through a port the node lacks",
                ))?;
            self.world.occupants[node.index()].retain(|&other| other != id);
            self.world.agents[id.index()].position = Position::InTransit {
                from: node,
                to: edge.neighbor,
                arrival_port: edge.remote_port,
            };
            self.metrics.agent_moves(1);
            self.changed = true;
        }
        for note in action.notes {
            self.record(node, note)?;
        }
        Ok(())
    }

    /// The simulator's own record of homes, and the two ways a decision can be
    /// caught out by it: settling where someone already lives, and a vacated
    /// agent resettling anywhere but its home.
    fn settle_check(
        &mut self,
        id: AgentId,
        before: AgentState,
        after: AgentState,
        node: NodeId,
    ) -> Result<(), RootedAsyncError> {
        match (before, after) {
            (AgentState::Unsettled, AgentState::Settled) => {
                if self.world.home_owner[node.index()].is_some() {
                    return Err(RootedAsyncError::NodeClaimedTwice(node));
                }
                self.world.home_owner[node.index()] = Some(id);
                self.world.agents[id.index()].home = Some(node);
                self.metrics.settlement();
            }
            (AgentState::SettledScout, AgentState::Settled)
                if self.world.agents[id.index()].home != Some(node) =>
            {
                return Err(RootedAsyncError::SettledAwayFromHome { agent: id, node });
            }
            _ => {}
        }
        Ok(())
    }

    fn record(&mut self, node: NodeId, event: Note) -> Result<(), RootedAsyncError> {
        match event {
            Note::PortProbe => {
                self.metrics.port_probe();
                self.metrics.scout_operation();
            }
            Note::ProbeOut => self.metrics.probe_out_traversal(),
            Note::ProbeBack => self.metrics.probe_back_traversal(),
            Note::Vacate => self.metrics.vacate(),
            Note::Backtrack => self.metrics.backtrack(),
            Note::RetraceMove => self.metrics.retrace(),
            Note::GroupMove(size) => self.metrics.group_move(size),
            Note::MacroRound => self.metrics.macro_round(),
            Note::Step {
                forward,
                batches,
                scouts,
            } => self.steps.push(HeadStep {
                epoch: self.epoch,
                node,
                forward,
                batches,
                scouts,
            }),
            Note::Constructed => self.constructed_at = Some(self.epoch),
            Note::HeadOwner(expected) => {
                let actual = self.world.home_owner[node.index()];
                if actual != Some(expected) {
                    return Err(RootedAsyncError::WrongHead {
                        node,
                        expected,
                        actual,
                    });
                }
            }
            Note::Probed { head, resolution } => {
                let neighbor = self
                    .world
                    .graph
                    .traverse(head, resolution.port)
                    .ok_or(RootedAsyncError::Protocol("probed a port the head lacks"))?
                    .neighbor;
                let actual = self.world.home_owner[neighbor.index()];
                if actual != resolution.owner {
                    return Err(RootedAsyncError::LocalViewDisagrees {
                        head,
                        port: resolution.port,
                        rule: resolution.rule,
                        local: resolution.owner,
                        actual,
                    });
                }
                if self.recorder.enabled() {
                    self.recorder.record_event(
                        self.epoch,
                        SimulationEvent::ProbeConcluded {
                            head,
                            port: resolution.port,
                            rule: resolution.rule.code(),
                            owner: resolution.owner,
                        },
                    );
                }
                self.probes.push(ProbeEvent {
                    epoch: self.epoch,
                    head,
                    port: resolution.port,
                    rule: resolution.rule,
                    owner: resolution.owner,
                });
            }
            Note::Stuck(reason) => return Err(RootedAsyncError::Protocol(reason)),
        }
        Ok(())
    }

    fn measure(&mut self, id: AgentId) {
        let bits = self.widths.memory(&self.world.agents[id.index()].memory);
        self.memory_bits = self.memory_bits.max(bits);
    }

    fn finished(&self) -> bool {
        self.world.agents.iter().all(|agent| {
            let memory = &agent.memory;
            memory.state == AgentState::Settled
                && memory.task == super::memory::Task::Idle
                && memory.lead.is_none()
                && agent
                    .home
                    .is_some_and(|home| agent.position == Position::At(home))
        })
    }

    fn tree(&self) -> Vec<TreeEdge> {
        let agents = &self.world.agents;
        agents
            .iter()
            .filter_map(|agent| {
                let child = agent.home?;
                let (parent_agent, port_at_parent) = agent.memory.parent?;
                let child_port = agent.memory.parent_port?;
                Some(TreeEdge {
                    parent: agents[parent_agent.index()].home?,
                    child,
                    child_port,
                    edge_type: EdgeType::of(child_port, port_at_parent),
                })
            })
            .collect()
    }

    fn checkpoint(&mut self) {
        if !self.recorder.enabled() {
            return;
        }
        let agents = &self.world.agents;
        let settled_agents = self
            .world
            .home_owner
            .iter()
            .map(|owner| {
                owner.filter(|id| {
                    let agent = &agents[id.index()];
                    agent.memory.state == AgentState::Settled
                        && agent
                            .home
                            .is_some_and(|home| agent.position == Position::At(home))
                })
            })
            .collect();
        self.recorder.record_checkpoint(Checkpoint {
            step: self.epoch,
            agent_nodes: agents.iter().map(|a| a.position.node()).collect(),
            agent_statuses: agents
                .iter()
                .map(|a| match a.memory.state {
                    AgentState::Unsettled => AgentStatus::Unsettled,
                    AgentState::Settled => AgentStatus::Settled,
                    AgentState::SettledScout => AgentStatus::SettledScout,
                })
                .collect(),
            home_nodes: agents.iter().map(|a| a.home).collect(),
            settled_agents,
            transit_from: agents
                .iter()
                .map(|a| match a.position {
                    Position::InTransit { from, .. } => Some(from),
                    Position::At(_) => None,
                })
                .collect(),
        });
    }
}

fn agent(index: usize) -> AgentId {
    AgentId(u32::try_from(index).expect("agent IDs fit the dense domain"))
}
