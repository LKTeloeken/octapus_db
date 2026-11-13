//! A árvore da TUI: servidores → databases → schemas → tabelas, carregada um
//! nível por vez, quando o nó abre (nunca a estrutura inteira).
//!
//! Os nós ficam numa arena; a lista do que está visível (`visible`) só é
//! refeita quando algo abre, fecha ou o filtro muda — o desenho percorre só
//! as linhas que cabem na tela, então 5.000 schemas custam o mesmo que 50.

use octapus_core::models::{DatabaseType, TableType};

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Server {
        id: i64,
        db_type: DatabaseType,
    },
    Database {
        server_id: i64,
        name: String,
    },
    Schema {
        server_id: i64,
        database: String,
        name: String,
    },
    Table {
        server_id: i64,
        database: String,
        schema: Option<String>,
        name: String,
        table_type: TableType,
    },
}

/// Onde uma consulta roda: tirado do nó selecionado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub server_id: i64,
    /// `None` = o database padrão do servidor
    pub database: Option<String>,
    pub schema: Option<String>,
}

impl NodeKind {
    pub fn is_leaf(&self) -> bool {
        matches!(self, NodeKind::Table { .. })
    }

    pub fn context(&self) -> Context {
        match self {
            NodeKind::Server { id, .. } => Context {
                server_id: *id,
                database: None,
                schema: None,
            },
            NodeKind::Database { server_id, name } => Context {
                server_id: *server_id,
                database: Some(name.clone()),
                schema: None,
            },
            NodeKind::Schema {
                server_id,
                database,
                name,
            } => Context {
                server_id: *server_id,
                database: Some(database.clone()),
                schema: Some(name.clone()),
            },
            NodeKind::Table {
                server_id,
                database,
                schema,
                ..
            } => Context {
                server_id: *server_id,
                database: Some(database.clone()),
                schema: schema.clone(),
            },
        }
    }

    /// O símbolo antes do nome (o tipo do banco, da relação…).
    pub fn glyph(&self) -> &'static str {
        match self {
            NodeKind::Server { db_type, .. } => match db_type {
                DatabaseType::Postgres => "pg",
                DatabaseType::Sqlite => "sl",
                DatabaseType::Mongodb => "mg",
                DatabaseType::Redis => "rd",
                DatabaseType::Mysql => "my",
            },
            NodeKind::Database { .. } => "▪",
            NodeKind::Schema { .. } => "◇",
            NodeKind::Table { table_type, .. } => match table_type {
                TableType::Table => "▤",
                TableType::View | TableType::MaterializedView => "◫",
                TableType::Foreign => "⇄",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Ainda não pedido
    Idle,
    Loading,
    Loaded,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub label: String,
    pub depth: u16,
    pub expanded: bool,
    pub load: Load,
    pub children: Vec<usize>,
    /// O erro do último carregamento, para a barra de status
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct Tree {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    /// Os nós visíveis, na ordem da tela
    visible: Vec<usize>,
    pub selected: usize,
    pub scroll: usize,
    filter: String,
}

impl Tree {
    pub fn set_roots(&mut self, roots: Vec<(NodeKind, String)>) {
        self.nodes.clear();
        self.roots = roots.into_iter().map(|(kind, label)| self.push(kind, label, 0)).collect();
        self.selected = 0;
        self.scroll = 0;
        self.rebuild();
    }

    fn push(&mut self, kind: NodeKind, label: String, depth: u16) -> usize {
        self.nodes.push(Node {
            kind,
            label,
            depth,
            expanded: false,
            load: Load::Idle,
            children: Vec::new(),
            error: None,
        });
        self.nodes.len() - 1
    }

    pub fn len(&self) -> usize {
        self.visible.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// O nó da linha `row` da lista visível.
    pub fn at(&self, row: usize) -> Option<(usize, &Node)> {
        let id = *self.visible.get(row)?;
        Some((id, self.nodes.get(id)?))
    }

    pub fn selected_node(&self) -> Option<(usize, &Node)> {
        self.at(self.selected)
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn set_filter(&mut self, filter: &str) {
        let keep = self.selected_node().map(|(id, _)| id);
        self.filter = filter.to_lowercase();
        self.rebuild();
        self.reselect(keep);
    }

    /// Marca o nó como carregando; `false` se já carregou ou está carregando.
    pub fn begin_load(&mut self, id: usize) -> bool {
        match self.nodes.get_mut(id) {
            Some(node) if matches!(node.load, Load::Idle | Load::Failed) => {
                node.load = Load::Loading;
                node.error = None;
                node.expanded = true;
                self.rebuild();
                true
            }
            _ => false,
        }
    }

    /// Os filhos chegaram: o nó abre com eles.
    pub fn set_children(&mut self, id: usize, children: Vec<(NodeKind, String)>) {
        let Some(depth) = self.nodes.get(id).map(|node| node.depth + 1) else {
            return;
        };
        let keep = self.selected_node().map(|(id, _)| id);
        let ids: Vec<usize> = children.into_iter().map(|(kind, label)| self.push(kind, label, depth)).collect();
        if let Some(node) = self.nodes.get_mut(id) {
            node.children = ids;
            node.load = Load::Loaded;
            node.expanded = true;
        }
        self.rebuild();
        self.reselect(keep);
    }

    pub fn set_failed(&mut self, id: usize, error: String) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.load = Load::Failed;
            node.expanded = false;
            node.error = Some(error);
        }
        self.rebuild();
    }

    /// Esquece os filhos (recarregar): o próximo abrir busca de novo.
    pub fn reset(&mut self, id: usize) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.children.clear();
            node.load = Load::Idle;
            node.expanded = false;
        }
        self.rebuild();
    }

    pub fn set_expanded(&mut self, id: usize, expanded: bool) {
        let keep = self.selected_node().map(|(id, _)| id);
        if let Some(node) = self.nodes.get_mut(id) {
            node.expanded = expanded;
        }
        self.rebuild();
        self.reselect(keep);
    }

    /// O pai de um nó (o nó visível anterior com profundidade menor).
    pub fn parent_row(&self, row: usize) -> Option<usize> {
        let depth = self.at(row)?.1.depth;
        (0..row).rev().find(|candidate| self.at(*candidate).is_some_and(|(_, node)| node.depth < depth))
    }

    pub fn move_by(&mut self, delta: isize) {
        let last = self.visible.len().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, last.max(0)) as usize;
    }

    pub fn select(&mut self, row: usize) {
        self.selected = row.min(self.visible.len().saturating_sub(1));
    }

    /// Ajusta a rolagem para a seleção caber em `height` linhas.
    pub fn scroll_into_view(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
        self.scroll = self.scroll.min(self.visible.len().saturating_sub(height));
    }

    fn reselect(&mut self, keep: Option<usize>) {
        let row = keep.and_then(|id| self.visible.iter().position(|visible| *visible == id));
        self.selected = row.unwrap_or(self.selected).min(self.visible.len().saturating_sub(1));
    }

    /// Refaz a lista do que aparece. Com filtro, ficam os nós cujo nome casa
    /// e os ancestrais deles; sem filtro, tudo que está aberto.
    fn rebuild(&mut self) {
        let mut visible = Vec::with_capacity(self.visible.len());
        let filter = self.filter.as_str();
        let mut stack: Vec<usize> = self.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let node = &self.nodes[id];
            if !filter.is_empty() && !self.matches(id, filter) {
                continue;
            }
            visible.push(id);
            if node.expanded {
                stack.extend(node.children.iter().rev().copied());
            }
        }
        self.visible = visible;
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }

    /// O nó ou algum descendente carregado casa com o filtro? Um nó ainda
    /// não aberto sempre fica (não se sabe o que há dentro).
    fn matches(&self, id: usize, filter: &str) -> bool {
        let node = &self.nodes[id];
        if node.label.to_lowercase().contains(filter) {
            return true;
        }
        if !node.kind.is_leaf() && node.load != Load::Loaded {
            return node.depth == 0;
        }
        node.children.iter().any(|child| self.matches(*child, filter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: i64) -> (NodeKind, String) {
        (
            NodeKind::Server {
                id,
                db_type: DatabaseType::Postgres,
            },
            format!("srv{id}"),
        )
    }

    fn schema(name: &str) -> (NodeKind, String) {
        (
            NodeKind::Schema {
                server_id: 1,
                database: "app".into(),
                name: name.into(),
            },
            name.into(),
        )
    }

    fn labels(tree: &Tree) -> Vec<String> {
        (0..tree.len()).map(|row| tree.at(row).unwrap().1.label.clone()).collect()
    }

    #[test]
    fn children_load_once_and_selection_follows_the_node() {
        let mut tree = Tree::default();
        tree.set_roots(vec![server(1), server(2)]);
        tree.select(1);

        assert!(tree.begin_load(0));
        assert!(!tree.begin_load(0), "já carregando");
        tree.set_children(0, vec![schema("public"), schema("tenant_1")]);
        assert_eq!(labels(&tree), ["srv1", "public", "tenant_1", "srv2"]);
        // A seleção continua no srv2, que desceu duas linhas
        assert_eq!(tree.selected_node().unwrap().1.label, "srv2");
        assert_eq!(tree.parent_row(2), Some(0));

        tree.set_expanded(0, false);
        assert_eq!(labels(&tree), ["srv1", "srv2"]);
        tree.reset(0);
        assert!(tree.begin_load(0), "recarrega depois do reset");
    }

    #[test]
    fn filter_keeps_matches_and_their_ancestors() {
        let mut tree = Tree::default();
        tree.set_roots(vec![server(1), server(2)]);
        tree.begin_load(0);
        tree.set_children(0, vec![schema("public"), schema("tenant_1"), schema("tenant_2")]);

        tree.set_filter("TENANT");
        // srv2 ainda não foi aberto: fica, não se sabe o que há dentro
        assert_eq!(labels(&tree), ["srv1", "tenant_1", "tenant_2", "srv2"]);
        tree.set_filter("");
        assert_eq!(tree.len(), 5);
    }

    #[test]
    fn scroll_keeps_the_selection_on_screen_with_thousands_of_rows() {
        let mut tree = Tree::default();
        tree.set_roots(vec![server(1)]);
        tree.begin_load(0);
        tree.set_children(0, (0..5000).map(|index| schema(&format!("tenant_{index:04}"))).collect());

        tree.move_by(4000);
        tree.scroll_into_view(20);
        assert_eq!(tree.selected, 4000);
        assert_eq!(tree.scroll, 3981);
        tree.move_by(-10_000);
        tree.scroll_into_view(20);
        assert_eq!((tree.selected, tree.scroll), (0, 0));
        tree.move_by(isize::MAX / 2);
        assert_eq!(tree.selected, 5000);
    }
}
