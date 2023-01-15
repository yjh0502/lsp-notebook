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

fn info_string(node: Node, content: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "info_string" {
            return child.utf8_text(content.as_bytes()).unwrap().to_string();
        }
    }
    String::new()
}

fn collect_codeblocks<'a, 'b>(node: Node<'a>, content: &'b str, actions: &mut Vec<Node<'a>>) {
    if node.kind() == "fenced_code_block" {
        actions.push(node);
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_codeblocks(child, content, actions);
    }
}

pub fn code_content(node: Node, content: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "code_fence_content" {
            continue;
        }

        return child.utf8_text(content.as_bytes()).unwrap().to_owned();
    }
    String::new()
}

pub fn parse(content: &str) -> Tree {
    let mut parser = parser();
    parser.parse(&content, None).unwrap()
}

pub fn code_actions<'a, 'b>(tree: &'a Tree, content: &'b str) -> Vec<(Node<'a>, Option<Node<'a>>)> {
    let mut actions = Vec::new();
    collect_codeblocks(tree.root_node(), content, &mut actions);

    let mut pairs = vec![];
    let mut i = 0;

    while i < actions.len() {
        let node = actions[i];
        i += 1;
        if i == actions.len() {
            pairs.push((node, None));
            break;
        }
        let next = actions[i];
        let info_str = info_string(next, content);
        if info_str == "output" {
            pairs.push((node, Some(next)));
            i += 1;
        } else {
            pairs.push((node, None));
        }
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_actions() {
        let content = r#"
```sh
echo hello
```
```output
hello
```
"#;

        let tree = parse(content);
        let actions = code_actions(&tree, content);
        assert_eq!(actions.len(), 1);
    }
}
