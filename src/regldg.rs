use std::iter::Peekable;

#[derive(Clone)]
enum SplitType {
    General,
    Alternation,
}

#[derive(Clone)]
enum NodeType {
    General,
    Leaf,
    Group,
}

#[derive(Clone)]
enum CharType {
    Undefined,
    Char,
    EscapeSequence,
    Nsc,
    Mcc,
    Backref,
    GroupStart,
    GroupFinish,
    Quantifier,
    BracedQuantifierStart,
    BracedQuantifierFinish,
    CharClassStart,
    CharClassFinish,
    Alternation,
    Range,
    RangeEnd,
    Control,
    NegateCharClass,
    Metachar,
}

#[derive(Clone)]
pub struct Node<'a> {
    split_type: SplitType,
    node_type: NodeType,
    parsing_alt_list: Vec<usize>,
    parsing_alt_pos_list: Vec<usize>,
    group_list: Vec<usize>,
    alternation_id: usize,
    tnode_id: usize,
    set: CharSet<'a>,
    children: Vec<Box<Node<'a>>>,
}

impl<'a> Node<'a> {
    pub fn new(
        tnode_id: usize,
        node_type: NodeType,
        set: CharSet<'a>,
        parsing_alt_list: Vec<usize>,
        parsing_alt_pos_list: Vec<usize>,
        group_list: Vec<usize>,
    ) -> Self {
        Self {
            split_type: SplitType::General,
            node_type: NodeType::General,
            parsing_alt_list,
            parsing_alt_pos_list,
            group_list,
            alternation_id: 0,
            tnode_id,
            set: CharSet::new(&[][..]),
            children: Vec::new(),
        }
    }

    fn add_child(&mut self, node: Box<Node<'a>>) {
        self.children.push(node);
    }
}

#[derive(Clone)]
pub struct CharSet<'a> {
    set: &'a [u8],
    pos: usize,
    ancestral_offset: usize,
}

impl<'a> CharSet<'a> {
    pub fn new(set: &'a [u8]) -> Self {
        Self {
            set,
            pos: 0,
            ancestral_offset: 0,
        }
    }
}

pub struct State<'a> {
    last_chartype_parsed: CharType,
    last_value_parsed: u8,

    // Group
    num_groups_completed: usize,
    num_groups_started: usize,
    group_start_pos: Vec<usize>,
    last_group_started: Vec<usize>,
    finished_groups: Vec<usize>,
    in_group: bool,

    current_atom_start_pos: usize,

    // Universe
    universe_check_code: usize,
    universe: CharSet<'a>,

    // Stop chars
    stop: &'a [u8],

    // Parsing
    parsing_alt_list: Vec<usize>,
    parsing_alt_pos_list: Vec<usize>,
    num_alternation_string: usize,

    // Nodes
    tnode_id: usize,
}

impl<'a> State<'a> {
    pub fn new() -> Self {
        Self {
            last_chartype_parsed: CharType::Undefined,
            last_value_parsed: 0,
            num_groups_completed: 0,
            num_groups_started: 0,
            group_start_pos: Vec::new(),
            last_group_started: Vec::new(),
            finished_groups: Vec::new(),
            in_group: false,
            current_atom_start_pos: 0,
            universe_check_code: 0,
            universe: CharSet::new(&[][..]),
            stop: &[b'|', b')'][..],
            parsing_alt_list: Vec::new(),
            parsing_alt_pos_list: Vec::new(),
            num_alternation_string: 0,
            tnode_id: 0,
        }
    }
}

pub fn parse_regex<'a>(
    c: &mut CharSet<'a>,
    state: &mut State,
    node: Option<Node<'a>>,
) -> Option<Node<'a>> {
    let mut alt_count = 0;
    let mut pos = c.pos;
    let perm_pos = pos;
    state.stop = if state.in_group {
        &[b'|'][..]
    } else {
        &[b'|', b')'][..]
    };

    let node =  scan_string(c, state, node, b'|');
    if let Some(mut n) = scan_string(c, state, node, b'|') {
        n.split_type = SplitType::Alternation;
        n.alternation_id = state.num_alternation_string;
        state.parsing_alt_list.push(state.num_alternation_string);
        state.num_alternation_string = state.num_alternation_string + 1;
    }
    loop {
        if let Some(n) = scan_string(c, state, node, b'|') {
            state.parsing_alt_pos_list.push(alt_count);
            alt_count = alt_count + 1;
            let child = Box::new(Node::new(
                state.tnode_id,
                NodeType::General,
                CharSet::new(&c.set[pos..c.pos - pos]),
                state.parsing_alt_list.drain(..).collect(),
                state.parsing_alt_pos_list.drain(..).collect(),
                state.last_group_started.drain(..).collect(),
            ));
            state.tnode_id = state.tnode_id + 1;
            (0..pos - c.pos).for_each(|_| {
                c.pos = c.pos + 1;
            });
            n.add_child(child);
            scan(c, state, node);
        } else {
            c.pos = c.pos + (pos - c.pos);
            scan(c, state, node);
            break;
        }
    }
    node = pass_alternation(c, state, node);
    pos = c.pos;
    if let Some(n) = node {
        state.parsing_alt_pos_list.push(alt_count);
        alt_count = alt_count + 1;
        let child = Box::new(Node::new(
            state.tnode_id,
            NodeType::General,
            CharSet::new(&c.set[pos..c.pos - pos]),
            state.parsing_alt_list.drain(..).collect(),
            state.parsing_alt_pos_list.drain(..).collect(),
            state.last_group_started.drain(..).collect(),
        ));
        state.tnode_id = state.tnode_id + 1;
        c.pos = c.pos + (pos - c.pos);
        n.add_child(child);
        node = scan(c, state, node);
        n.set.set = &c.set[n.set.pos..perm_pos][..];
    } else {
        c.pos = c.pos + (pos - c.pos);
        node = scan(c, state, node);
    }
    node
}

fn scan_string<'a>(
    c: &mut CharSet<'a>,
    state: &mut State,
    node: Option<Node<'a>>,
    ch: u8,
) -> Option<Node<'a>> {
    if scan(c, state, node).is_some() && unsafe { *c.set.get_unchecked(c.pos) == ch } {
        node
    } else {
        None
    }
}

fn print_regex<'a>(c: &CharSet, state: &State, msg: &str) {
    use std::str::from_utf8;
    println!("{}", from_utf8(c.set).unwrap());
    (0..c.ancestral_offset + state.current_atom_start_pos).for_each(|_| print!(" "));
    (c.ancestral_offset + state.current_atom_start_pos..c.ancestral_offset + c.pos)
        .for_each(|_| print!("^"));
    println!("{}", msg);
}

fn pass_char<'a>(c: &CharSet, state: &mut State, node: Option<Node<'a>>) -> Option<Node<'a>> {
    if let Some(&ch) = c.set.get(c.pos) {
        if state.universe_check_code > 0 {
            if state.universe.set.iter().find(|&s| *s == ch).is_some() {
                c.pos = c.pos + 1;
                print_regex(c, state, "specified character not in universe");
                panic!("specified character not in universe {}", ch);
            }
        } else {
            state.last_chartype_parsed = CharType::Char;
            state.last_value_parsed = ch;
            c.pos = c.pos + 1;
        }
    }
    node
}

fn pass_alternation<'a>(
    c: &mut CharSet<'a>,
    state: &mut State,
    node: Option<Node<'a>>,
) -> Option<Node<'a>> {
    match state.last_chartype_parsed {
        CharType::Char
        | CharType::Nsc
        | CharType::Mcc
        | CharType::Backref
        | CharType::GroupFinish
        | CharType::Quantifier
        | CharType::BracedQuantifierFinish
        
        | CharType::Control
        | CharType::Metachar
        | CharType::CharClassFinish => (),
        _ => print_regex(
            c,
            state,
            "parse_regex_pass_alternation: character type preceeding alternation is invalid",
        ),
    }
    state.current_atom_start_pos = c.pos;
    c.pos = c.pos + 1;
    state.last_chartype_parsed = CharType::Alternation;
    if node.is_some() {
        print_regex(c, state, "Parsed alternation");
    }
    node
}

fn scan<'a>(c: &mut CharSet<'a>, state: &mut State, node: Option<Node<'a>>) -> Option<Node<'a>> {
    let mut chars_read = 0;

    while let Some(ch) = c.set.get(c.pos) {
        // End group return
        if state.stop.iter().find(|&s| *s == b')').is_some() {
            if chars_read == 0 {
                assert!(false);
            } else {
                state.last_chartype_parsed = CharType::GroupFinish;
                return node;
            }
        }
        if state.stop.iter().find(|&s| ch == s).is_some() {
            return node;
        }
        match ch {
            b'\\' => node,
            b'(' => {
                state.num_groups_started + state.num_groups_started + 1;
                state.last_chartype_parsed = CharType::GroupStart;
                c.pos = c.pos + 1;

                if let Some(_) = node {
                    println!("Started group {}", state.num_groups_started);
                    print_regex(c, state, "starting group")
                } else {
                    state.in_group = true;
                    parse_regex(c, state, node);
                }
                return node;
            }
            b'*' | b'+' | b'?' | b'{' => node,
            b'[' => node,
            b'.' => node,
            b'|' => node,
            b']' => node,
            b')' => node,
            b'}' => node,
            _ => {
                let node = pass_char(c, state, node);
                if node.is_some() {
                    print_regex(c, state, "regular char");
                    //add_child_node(c, node);
                }
                chars_read = chars_read + 1;
                node
            }
        }
    }
    None
}
