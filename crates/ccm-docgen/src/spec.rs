//! The flowcharts, as data.
//!
//! Nodes carry grid coordinates rather than being laid out automatically. A
//! solver would have to be told where to put things anyway to read well at this
//! size, and placing them by hand keeps the diagrams stable: adding a node does
//! not shuffle the rest.

/// What a box means, which decides its shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// Rounded pill: an entry point.
    Entry,
    /// Rectangle: a step. Usually a function.
    Step,
    /// Diamond: a branch.
    Decision,
    /// Pill: a terminal state.
    Terminal,
    /// Dashed rectangle: a helper called from the step beside it.
    Aside,
}

pub struct Node {
    pub id: &'static str,
    /// Text in the box. `|` separates lines.
    pub label: &'static str,
    pub col: f64,
    pub row: f64,
    pub shape: Shape,
    /// Function this box stands for, if clicking it should show code. The name
    /// must exist in the chart's source file, or in `file` if that is set.
    pub func: Option<&'static str>,
    /// The file `func` is in, for an algorithm whose code spans several.
    pub file: Option<&'static str>,
}

impl Node {
    /// The same box, with its function looked up in `file`.
    const fn in_file(self, file: &'static str) -> Self {
        Self {
            file: Some(file),
            ..self
        }
    }
}

/// How an edge gets from one box to another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    /// Straight line between the nearest sides.
    Direct,
    /// Out of the source's side, around at `x`, and back into the target.
    Around(i32),
}

pub struct Edge {
    pub from: &'static str,
    pub to: &'static str,
    pub label: Option<&'static str>,
    pub route: Route,
    /// Drawn dashed: a call rather than a step in the flow.
    pub aside: bool,
}

pub struct Chart {
    pub id: &'static str,
    pub title: &'static str,
    pub crate_name: &'static str,
    pub source: &'static str,
    pub blurb: &'static str,
    pub nodes: &'static [Node],
    pub edges: &'static [Edge],
}

const fn n(
    id: &'static str,
    label: &'static str,
    col: f64,
    row: f64,
    shape: Shape,
    func: Option<&'static str>,
) -> Node {
    Node {
        id,
        label,
        col,
        row,
        shape,
        func,
        file: None,
    }
}

const fn e(
    from: &'static str,
    to: &'static str,
    label: Option<&'static str>,
    route: Route,
) -> Edge {
    Edge {
        from,
        to,
        label,
        route,
        aside: false,
    }
}

const fn aside(from: &'static str, to: &'static str, label: Option<&'static str>) -> Edge {
    Edge {
        from,
        to,
        label,
        route: Route::Direct,
        aside: true,
    }
}

pub static CHARTS: &[Chart] = &[
    Chart {
        id: "drop",
        title: "Drop and Freeze",
        crate_name: "ccm-algorithms",
        source: "crates/ccm-algorithms/src/drop_and_freeze.rs",
        blurb: "Three phases per round. Unsettled agents step out one per port and come back \
                with one bit: is a settled agent there? On a node with no settler the \
                lowest-ID agent settles. The rest move through the first empty port at or \
                after the settler's cursor; if there is none the cursor moves on, and only once \
                every port has been tried does the group backtrack along the settler's parent \
                port. Several start nodes are allowed, and groups that meet merge.",
        nodes: &[
            n(
                "start",
                "simulate_with()",
                0.0,
                0.0,
                Shape::Entry,
                Some("simulate_with"),
            ),
            n(
                "canon",
                "canonicalize_ports()",
                0.0,
                1.0,
                Shape::Step,
                Some("canonicalize_ports"),
            ),
            n("settled", "all settled?", 0.0, 2.0, Shape::Decision, None),
            n("done", "Completed", 1.5, 2.0, Shape::Terminal, None),
            n("limit", "rounds left?", 0.0, 3.0, Shape::Decision, None),
            n(
                "capped",
                "RoundLimitReached",
                1.5,
                3.0,
                Shape::Terminal,
                None,
            ),
            n(
                "pout",
                "probe_out()",
                0.0,
                4.0,
                Shape::Step,
                Some("probe_out"),
            ),
            n(
                "ports",
                "ordered_ports()",
                1.3,
                4.0,
                Shape::Aside,
                Some("ordered_ports"),
            ),
            n(
                "pback",
                "probe_back()|bit: settler there?",
                0.0,
                5.0,
                Shape::Step,
                Some("probe_back"),
            ),
            n(
                "settle",
                "move_out(): lowest ID|settles if no settler",
                0.0,
                6.0,
                Shape::Step,
                Some("move_out"),
            ),
            n(
                "found",
                "empty port at|or after cursor?",
                0.0,
                7.0,
                Shape::Decision,
                None,
            ),
            n(
                "fwd",
                "move the group|through it",
                1.3,
                7.0,
                Shape::Step,
                None,
            ),
            n(
                "count",
                "cursor += ports|probed",
                0.0,
                8.0,
                Shape::Step,
                None,
            ),
            n(
                "scouts",
                "scouts_for_count()",
                1.3,
                8.0,
                Shape::Aside,
                Some("scouts_for_count"),
            ),
            n(
                "tried",
                "every port|tried?",
                0.0,
                9.0,
                Shape::Decision,
                None,
            ),
            n(
                "stay",
                "stay; probe the|next ports",
                -1.5,
                10.5,
                Shape::Step,
                None,
            ),
            n("parent", "parent port?", 0.0, 10.0, Shape::Decision, None),
            n(
                "back",
                "backtrack along|the parent port",
                0.0,
                11.0,
                Shape::Step,
                None,
            ),
            n(
                "valid",
                "validate_state()",
                0.0,
                12.0,
                Shape::Step,
                Some("validate_state"),
            ),
        ],
        edges: &[
            e("start", "canon", None, Route::Direct),
            e("canon", "settled", None, Route::Direct),
            e("settled", "done", Some("yes"), Route::Direct),
            e("settled", "limit", Some("no"), Route::Direct),
            e("limit", "capped", Some("no"), Route::Direct),
            e("limit", "pout", Some("yes"), Route::Direct),
            e("pout", "pback", None, Route::Direct),
            e("pback", "settle", None, Route::Direct),
            e("settle", "found", None, Route::Direct),
            e("found", "fwd", Some("yes"), Route::Direct),
            e("found", "count", Some("no"), Route::Direct),
            e("count", "tried", None, Route::Direct),
            e("tried", "stay", Some("no"), Route::Direct),
            e("tried", "parent", Some("yes"), Route::Direct),
            e("parent", "back", Some("yes"), Route::Direct),
            e("parent", "stay", Some("no: start node"), Route::Direct),
            e("back", "valid", None, Route::Direct),
            e("stay", "valid", None, Route::Direct),
            e("fwd", "valid", None, Route::Around(1)),
            e("valid", "settled", Some("next round"), Route::Around(-2)),
            aside("pout", "ports", Some("port order")),
            aside("count", "scouts", Some("how many")),
        ],
    },
    Chart {
        id: "help",
        title: "Help by Scouts",
        crate_name: "ccm-algorithms",
        source: "crates/ccm-algorithms/src/help_by_scouts.rs",
        blurb: "Also a DFS, but settled agents help. Each round the group probes the head's \
                ports in parallel, and only then does the node's owner decide whether it can \
                vacate; if it can, it travels on with the group as a scout for the next probe. \
                When nobody is left unsettled, retrace walks the tree from where the group \
                stands and drops each scout at its home the first time it gets there. Rooted \
                starts only. Any step can hit the round limit, which ends the run there.",
        nodes: &[
            n("start", "run() / simulate()", 0.0, 0.0, Shape::Entry, None),
            n(
                "trans",
                "run_transitions()",
                0.0,
                1.0,
                Shape::Step,
                Some("run_transitions"),
            ),
            n("left", "unsettled left?", 0.0, 2.0, Shape::Decision, None),
            n(
                "settler",
                "settler at the|head's node?",
                0.0,
                3.0,
                Shape::Decision,
                None,
            ),
            n(
                "settle",
                "settle() highest|unsettled ID",
                1.35,
                3.0,
                Shape::Step,
                Some("settle"),
            ),
            n(
                "probe",
                "parallel_probe()|picks the port",
                0.0,
                4.0,
                Shape::Step,
                Some("parallel_probe"),
            ),
            n(
                "rank",
                "candidate_rank()",
                1.35,
                4.0,
                Shape::Aside,
                Some("candidate_rank"),
            ),
            n(
                "etype",
                "edge_type()",
                1.35,
                5.0,
                Shape::Aside,
                Some("edge_type"),
            ),
            n(
                "update",
                "update_node_type()",
                0.0,
                5.0,
                Shape::Step,
                Some("update_node_type"),
            ),
            n(
                "vacate",
                "can_vacate()",
                0.0,
                6.0,
                Shape::Decision,
                Some("can_vacate"),
            ),
            n(
                "scout",
                "owner joins the|group as a scout",
                1.35,
                6.0,
                Shape::Step,
                None,
            ),
            n("port", "port found?", 0.0, 7.0, Shape::Decision, None),
            n(
                "fwd",
                "move_group()|forward",
                1.35,
                8.0,
                Shape::Step,
                Some("move_group"),
            ),
            n(
                "adopt",
                "adopt the node if|partially visited",
                1.35,
                9.0,
                Shape::Step,
                None,
            ),
            n("parent", "parent port?", 0.0, 8.0, Shape::Decision, None),
            n("err", "MissingParent", -1.35, 8.0, Shape::Terminal, None),
            n(
                "back",
                "move_group()|to the parent",
                0.0,
                9.0,
                Shape::Step,
                None,
            ),
            n("next", "next round", 0.0, 10.0, Shape::Step, None),
            n(
                "retrace",
                "retrace()",
                -1.45,
                2.75,
                Shape::Step,
                Some("retrace"),
            ),
            n(
                "home",
                "settle_vacated_at()|on first visit",
                -1.45,
                3.75,
                Shape::Step,
                Some("settle_vacated_at"),
            ),
            n(
                "final",
                "validate_final()",
                -1.45,
                4.75,
                Shape::Step,
                Some("validate_final"),
            ),
            n("done", "Completed", -1.45, 5.75, Shape::Terminal, None),
        ],
        edges: &[
            e("start", "trans", None, Route::Direct),
            e("trans", "left", None, Route::Direct),
            e("left", "retrace", Some("no"), Route::Direct),
            e("left", "settler", Some("yes"), Route::Direct),
            e("settler", "probe", Some("yes"), Route::Direct),
            e("settler", "settle", Some("no"), Route::Direct),
            e("settle", "left", Some("check again"), Route::Around(2)),
            e("probe", "update", None, Route::Direct),
            e("update", "vacate", None, Route::Direct),
            e("vacate", "port", Some("settled"), Route::Direct),
            e("vacate", "scout", Some("settledScout"), Route::Direct),
            e("scout", "port", None, Route::Direct),
            e("port", "fwd", Some("yes"), Route::Direct),
            e("port", "parent", Some("no"), Route::Direct),
            e("parent", "back", Some("yes"), Route::Direct),
            e("parent", "err", Some("no: root"), Route::Direct),
            e("fwd", "adopt", None, Route::Direct),
            e("adopt", "next", None, Route::Direct),
            e("back", "next", None, Route::Direct),
            e("next", "left", None, Route::Around(-2)),
            e("retrace", "home", None, Route::Direct),
            e("home", "final", None, Route::Direct),
            e("final", "done", None, Route::Direct),
            aside("probe", "rank", Some("ranks ports")),
            aside("probe", "etype", Some("classify")),
        ],
    },
    Chart {
        id: "p1tree",
        title: "P1Tree",
        crate_name: "ccm-algorithms",
        source: "crates/ccm-algorithms/src/p1tree.rs",
        blurb: "A DFS that builds a port-one tree: every vertex ends up with an incident tree \
                edge carrying port 1 at one of its ends. Two things carry the O(k) bound: the \
                search stops at the first batch that finds somewhere to go, and the traversal \
                stops at dispersion rather than finishing the tree. Either way out, dispersion \
                or the root being popped, is followed by retrace, which walks the scouts home \
                along the tree. A run that hits the round limit stops without retrace.",
        nodes: &[
            n("start", "run() / simulate()", 0.0, 0.0, Shape::Entry, None),
            n(
                "root",
                "settle_highest_at(root)",
                0.0,
                1.0,
                Shape::Step,
                Some("settle_highest_at"),
            ),
            n(
                "trav",
                "traverse()",
                0.0,
                2.0,
                Shape::Step,
                Some("traverse"),
            ),
            n(
                "disp",
                "dispersed?",
                0.0,
                3.0,
                Shape::Decision,
                Some("note_dispersion"),
            ),
            n(
                "retrace",
                "retrace()",
                -1.5,
                3.0,
                Shape::Step,
                Some("retrace"),
            ),
            n(
                "post",
                "tour() of the tree,|drop scouts at home",
                -1.5,
                4.0,
                Shape::Step,
                Some("tour"),
            ),
            n("done", "Completed", -1.5, 5.0, Shape::Terminal, None),
            n(
                "search",
                "neighbourhood_search()",
                0.0,
                4.0,
                Shape::Step,
                Some("neighbourhood_search"),
            ),
            n(
                "party",
                "probe_party()|picks the scouts",
                0.0,
                5.0,
                Shape::Step,
                Some("probe_party"),
            ),
            n(
                "batch",
                "next batch: scouts|go out and back",
                0.0,
                6.0,
                Shape::Step,
                Some("probe_detour"),
            ),
            n(
                "found",
                "somewhere to go,|or no ports left?",
                0.0,
                7.0,
                Shape::Decision,
                None,
            ),
            n(
                "choose",
                "choose_next_edge()?",
                0.0,
                8.0,
                Shape::Decision,
                Some("choose_next_edge"),
            ),
            n(
                "defer",
                "must_defer()?",
                1.35,
                9.0,
                Shape::Decision,
                Some("must_defer"),
            ),
            n(
                "await",
                "awaits_port_one()?",
                -1.5,
                9.0,
                Shape::Decision,
                Some("awaits_port_one"),
            ),
            n(
                "partial",
                "mark|partiallyVisited",
                -0.15,
                10.0,
                Shape::Step,
                None,
            ),
            n("full", "mark fullyVisited", -1.5, 10.0, Shape::Step, None),
            n(
                "advance",
                "advance()",
                1.35,
                10.0,
                Shape::Step,
                Some("advance"),
            ),
            n(
                "canvac",
                "apply_can_vacate()",
                1.35,
                11.0,
                Shape::Step,
                Some("apply_can_vacate"),
            ),
            n(
                "parvac",
                "apply_parent_vacate()",
                1.35,
                12.0,
                Shape::Step,
                Some("apply_parent_vacate"),
            ),
            n(
                "vacate",
                "apply_can_vacate()",
                -0.8,
                11.0,
                Shape::Step,
                None,
            ),
            n(
                "backtrack",
                "backtrack()",
                -0.8,
                12.0,
                Shape::Step,
                Some("backtrack"),
            ),
            n("popped", "root popped?", -0.8, 13.0, Shape::Decision, None),
            n("next", "next step", 0.3, 14.0, Shape::Step, None),
        ],
        edges: &[
            e("start", "root", None, Route::Direct),
            e("root", "trav", None, Route::Direct),
            e("trav", "disp", None, Route::Direct),
            e("disp", "retrace", Some("yes"), Route::Direct),
            e("disp", "search", Some("no"), Route::Direct),
            e("retrace", "post", None, Route::Direct),
            e("post", "done", None, Route::Direct),
            e("search", "party", None, Route::Direct),
            e("party", "batch", None, Route::Direct),
            e("batch", "found", None, Route::Direct),
            e("found", "batch", Some("no"), Route::Around(1)),
            e("found", "choose", Some("yes"), Route::Direct),
            e("choose", "defer", Some("found"), Route::Direct),
            e("choose", "await", Some("exhausted"), Route::Direct),
            e("defer", "partial", Some("yes"), Route::Direct),
            e("defer", "advance", Some("no"), Route::Direct),
            e("await", "partial", Some("yes"), Route::Direct),
            e("await", "full", Some("no"), Route::Direct),
            e("advance", "canvac", None, Route::Direct),
            e("canvac", "parvac", None, Route::Direct),
            e("parvac", "next", None, Route::Direct),
            e("partial", "vacate", None, Route::Direct),
            e("full", "vacate", None, Route::Direct),
            e("vacate", "backtrack", None, Route::Direct),
            e("backtrack", "popped", None, Route::Direct),
            e("popped", "next", Some("no"), Route::Direct),
            e("popped", "retrace", Some("yes"), Route::Around(-2)),
            e("next", "disp", None, Route::Around(7)),
        ],
    },
    Chart {
        id: "rooted_async",
        title: "RootedAsync",
        crate_name: "ccm-algorithms",
        source: "crates/ccm-algorithms/src/rooted_async/lead.rs",
        blurb: "The same DFS run agent by agent. A schedule activates agents one cycle at a \
                time; in each, an agent reads only the agents at its node, writes its own memory \
                and at most one other, and moves along at most one port. One agent leads: it \
                settles the highest unsettled ID, sends the group out in probe batches, concludes \
                each probe by rules R1 to R3c from what the group remembers, and decides the next \
                edge and who vacates. Members carry out its numbered commands. The simulator \
                checks every probe's conclusion against the truth. When nobody is unsettled the \
                lead role passes to the scouts, who walk the tree home.",
        nodes: &[
            n(
                "start",
                "simulate()",
                0.0,
                0.0,
                Shape::Entry,
                Some("simulate"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/mod.rs"),
            n("sched", "schedule?", 0.0, 1.0, Shape::Decision, None),
            n(
                "sync",
                "run_synchronous():|every agent, one round",
                -1.5,
                2.0,
                Shape::Step,
                Some("run_synchronous"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/run.rs"),
            n(
                "rand",
                "run_asynchronous():|one agent at a time",
                0.0,
                2.0,
                Shape::Step,
                Some("run_asynchronous"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/run.rs"),
            n(
                "inter",
                "run_interleaved():|look now, write later",
                1.5,
                2.0,
                Shape::Step,
                Some("run_interleaved"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/run.rs"),
            n(
                "act",
                "activate():|one agent, one cycle",
                0.0,
                3.0,
                Shape::Step,
                Some("activate"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/agent.rs"),
            n("task", "lead, or task?", 0.0, 4.0, Shape::Decision, None),
            n(
                "follow",
                "follow(): carry out|the newest command",
                1.6,
                5.0,
                Shape::Step,
                Some("follow"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/agent.rs"),
            n(
                "probe",
                "probe_step():|one hop of R1-R3",
                1.6,
                6.0,
                Shape::Step,
                Some("probe_step"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/agent.rs"),
            n(
                "errand",
                "errand_step():|a V2-V5 excursion",
                1.6,
                7.0,
                Shape::Step,
                Some("errand_step"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/agent.rs"),
            n(
                "gather",
                "arrive(): settle the|highest ID, or D4",
                0.0,
                5.0,
                Shape::Step,
                Some("arrive"),
            ),
            n("left", "anyone|unsettled?", 0.0, 6.0, Shape::Decision, None),
            n(
                "plan",
                "plan_probe():|batches, scatter mode",
                0.0,
                7.0,
                Shape::Step,
                Some("plan_probe"),
            ),
            n(
                "batch",
                "finish_batch():|conclude each probe",
                0.0,
                8.0,
                Shape::Step,
                Some("finish_batch"),
            ),
            n(
                "resolve",
                "resolve(): R1-R3c",
                1.6,
                8.6,
                Shape::Aside,
                Some("resolve"),
            )
            .in_file("crates/ccm-algorithms/src/rooted_async/probe.rs"),
            n("more", "ports left?", 0.0, 9.0, Shape::Decision, None),
            n(
                "decide",
                "decide(): node type,|next edge, who vacates",
                0.0,
                10.0,
                Shape::Step,
                Some("decide"),
            ),
            n(
                "vacate",
                "errand_done():|V2-V5 outcome",
                0.0,
                11.0,
                Shape::Step,
                Some("errand_done"),
            ),
            n(
                "move",
                "begin_move(): group|leaves, lead last",
                0.0,
                12.0,
                Shape::Step,
                Some("begin_move"),
            ),
            n(
                "begin",
                "begin_retrace():|lead passes to a scout",
                -1.5,
                6.0,
                Shape::Step,
                Some("begin_retrace"),
            ),
            n(
                "rstep",
                "retrace_step():|next node home",
                -1.5,
                7.0,
                Shape::Step,
                Some("retrace_step"),
            ),
            n(
                "travel",
                "scouts still|travelling?",
                -1.5,
                8.0,
                Shape::Decision,
                None,
            ),
            n("done", "Completed", -1.5, 9.0, Shape::Terminal, None),
        ],
        edges: &[
            e("start", "sched", None, Route::Direct),
            e("sched", "sync", Some("synchronous"), Route::Direct),
            e("sched", "rand", Some("random"), Route::Direct),
            e("sched", "inter", Some("interleaved"), Route::Direct),
            e("sync", "act", None, Route::Direct),
            e("rand", "act", None, Route::Direct),
            e("inter", "act", None, Route::Direct),
            e("act", "task", None, Route::Direct),
            e("task", "gather", Some("lead"), Route::Direct),
            e("task", "follow", Some("member, idle"), Route::Direct),
            e("task", "probe", Some("probing"), Route::Direct),
            e("task", "errand", Some("errand"), Route::Direct),
            e("gather", "left", None, Route::Direct),
            e("left", "begin", Some("no"), Route::Direct),
            e("left", "plan", Some("yes"), Route::Direct),
            e("plan", "batch", None, Route::Direct),
            e("batch", "more", None, Route::Direct),
            e("more", "batch", Some("yes"), Route::Around(1)),
            e("more", "decide", Some("no"), Route::Direct),
            e("decide", "vacate", None, Route::Direct),
            e("vacate", "move", None, Route::Direct),
            e("move", "gather", Some("next node"), Route::Around(-1)),
            e("begin", "rstep", None, Route::Direct),
            e("rstep", "travel", None, Route::Direct),
            e("travel", "rstep", Some("yes"), Route::Around(-1)),
            e("travel", "done", Some("no"), Route::Direct),
            aside("batch", "resolve", Some("rules")),
        ],
    },
];
