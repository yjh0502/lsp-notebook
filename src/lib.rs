use tree_sitter::*;

extern "C" {
    fn tree_sitter_markdown() -> Language;
}

pub fn parser() -> Parser {
    let mut parser = Parser::new();

    let language = unsafe { tree_sitter_markdown() };
    parser.set_language(language).unwrap();

    parser
}

pub fn pos_ts_to_lsp(p: Point) -> lsp_types::Position {
    lsp_types::Position {
        line: p.row as u32,
        character: p.column as u32,
    }
}

pub fn node_range(node: Node) -> lsp_types::Range {
    lsp_types::Range {
        start: pos_ts_to_lsp(node.start_position()),
        end: pos_ts_to_lsp(node.end_position()),
    }
}

pub fn node_by_id(node: Node, id: usize) -> Option<Node> {
    if node.id() == id {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(node) = node_by_id(child, id) {
            return Some(node);
        }
    }
    None
}

fn collect_codes<'a>(node: Node<'a>, actions: &mut Vec<Node<'a>>) {
    if node.kind() == "fenced_code_block" {
        actions.push(node);
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_codes(child, actions);
    }
}

pub fn parse(content: &str) -> Tree {
    let mut parser = parser();
    parser.parse(&content, None).unwrap()
}

pub fn code_actions(tree: &Tree) -> Vec<Node> {
    let mut actions = Vec::new();
    collect_codes(tree.root_node(), &mut actions);
    actions
}
