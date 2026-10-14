//! The simulated world, and the local view one activation gets of it.
//!
//! An activation sees its own memory and the memories of agents at the same
//! node, and nothing else. It may rewrite its own memory and that of at most one
//! co-located agent, and may leave along one port: the paper's
//! Communicate-Compute-Move cycle (Section 2). [`Cx`] is the only way the
//! protocol touches the world, which is what keeps it local.

use super::memory::{AgentState, AsyncAgent, Memory, Position};
use super::probe::Resolution;
use ccm_core::{AgentId, NodeId, PortGraph, PortId};

pub(super) struct World<'g> {
    pub graph: &'g PortGraph,
    pub agents: Vec<AsyncAgent>,
    /// Agents standing at each node, by ascending ID.
    pub occupants: Vec<Vec<AgentId>>,
    /// Simulator only: the agent whose home each node is.
    pub home_owner: Vec<Option<AgentId>>,
    /// Simulator only: bumped whenever anything an agent at the node could
    /// see changes, so a commit can tell whether its look went stale.
    pub version: Vec<u64>,
}

impl World<'_> {
    pub fn memory(&self, id: AgentId) -> &Memory {
        &self.agents[id.index()].memory
    }

    pub fn at(&self, node: NodeId) -> impl Iterator<Item = &Memory> + '_ {
        self.occupants[node.index()]
            .iter()
            .map(|&id| &self.agents[id.index()].memory)
    }

    pub fn land(&mut self, id: AgentId) -> bool {
        let agent = &mut self.agents[id.index()];
        let Position::InTransit {
            to, arrival_port, ..
        } = agent.position
        else {
            return false;
        };
        agent.position = Position::At(to);
        agent.memory.arrival_port = Some(arrival_port);
        self.version[to.index()] += 1;
        let list = &mut self.occupants[to.index()];
        let at = list.partition_point(|&other| other < id);
        list.insert(at, id);
        true
    }
}

/// A side effect of an activation that the simulator records or checks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Note {
    PortProbe,
    ProbeOut,
    ProbeBack,
    Vacate,
    Backtrack,
    RetraceMove,
    GroupMove(usize),
    MacroRound,
    /// The group leaves the head: one step of the DFS.
    Step {
        forward: bool,
        batches: u16,
        scouts: u16,
    },
    /// Every agent has settled; Retrace begins.
    Constructed,
    /// The group expects this agent to own the node it has reached.
    HeadOwner(AgentId),
    /// A probe the head concluded, to be checked against the truth.
    Probed {
        head: NodeId,
        resolution: Resolution,
    },
    /// The protocol could not continue.
    Stuck(&'static str),
}

pub(super) struct Action {
    pub own: Memory,
    pub other: Option<Memory>,
    pub depart: Option<PortId>,
    pub notes: Vec<Note>,
}

/// One activation's view of the world.
pub(super) struct Cx<'w, 'g> {
    pub world: &'w World<'g>,
    pub node: NodeId,
    pub me: Memory,
    other: Option<Memory>,
    depart: Option<PortId>,
    pub notes: Vec<Note>,
}

impl<'w, 'g> Cx<'w, 'g> {
    pub fn new(world: &'w World<'g>, id: AgentId) -> Self {
        let agent = &world.agents[id.index()];
        let Position::At(node) = agent.position else {
            unreachable!("an agent is landed before it is activated");
        };
        Self {
            world,
            node,
            me: agent.memory.clone(),
            other: None,
            depart: None,
            notes: Vec::new(),
        }
    }

    pub fn finish(self) -> Action {
        Action {
            own: self.me,
            other: self.other,
            depart: self.depart,
            notes: self.notes,
        }
    }

    pub fn degree(&self) -> u16 {
        self.world.graph.degree(self.node).unwrap_or(0)
    }

    /// The other agents here, as this activation sees them.
    pub fn others(&self) -> impl Iterator<Item = &Memory> + '_ {
        let me = self.me.id;
        self.world
            .at(self.node)
            .filter(move |m| m.id != me)
            .map(|m| match &self.other {
                Some(written) if written.id == m.id => written,
                _ => m,
            })
    }

    /// `xi`: the agent settled and standing here, if any.
    pub fn settled_here(&self) -> Option<&Memory> {
        self.others().find(|m| m.state == AgentState::Settled)
    }

    /// The group as it stands here, this agent included if it belongs.
    pub fn group_here(&self) -> Vec<Memory> {
        let mut group: Vec<Memory> = self.others().filter(|m| m.in_group()).cloned().collect();
        if self.me.in_group() {
            group.push(self.me.clone());
        }
        group.sort_by_key(|m| m.id);
        group
    }

    pub fn find(&self, id: AgentId) -> Option<&Memory> {
        if id == self.me.id {
            return Some(&self.me);
        }
        self.others().find(|m| m.id == id)
    }

    /// Opens the one other agent this activation may write. Asking for a second
    /// agent is a protocol bug, not a modelling choice, so it panics.
    pub fn write(&mut self, id: AgentId) -> &mut Memory {
        assert_ne!(id, self.me.id, "an agent writes its own memory directly");
        if let Some(written) = &self.other {
            assert_eq!(written.id, id, "one other write per cycle");
        } else {
            let current = self
                .others()
                .find(|m| m.id == id)
                .expect("an agent only writes a co-located agent")
                .clone();
            self.other = Some(current);
        }
        self.other.as_mut().expect("just opened")
    }

    pub fn leave(&mut self, port: PortId) {
        debug_assert!(self.depart.is_none(), "one move per cycle");
        self.depart = Some(port);
    }

    pub fn note(&mut self, note: Note) {
        self.notes.push(note);
    }
}
