//! The boundary nesting tree and where each relation is routed in it.

use std::collections::HashMap;

use crate::parse::{C4Element, C4Relation};

/// One nesting level: the diagram root (index 0) or a boundary's body.
pub(super) struct Cluster<'a> {
    pub boundary: Option<&'a C4Element>,
    pub parent: Option<usize>,
    /// This cluster's item index within its parent.
    pub slot: Option<usize>,
    pub items: Vec<&'a C4Element>,
    /// Per item, the cluster a boundary item opens.
    pub opens: Vec<Option<usize>>,
}

pub(super) struct Tree<'a> {
    pub clusters: Vec<Cluster<'a>>,
    /// alias → (cluster holding it as a direct item, item index). The first
    /// declaration (in pre-order) wins, as in grid mode; later duplicates
    /// are still laid out and drawn but can't be a relation endpoint.
    pub home: HashMap<&'a str, (usize, usize)>,
}

impl<'a> Tree<'a> {
    pub fn build(elements: &'a [C4Element]) -> Self {
        let mut t = Tree {
            clusters: vec![Cluster {
                boundary: None,
                parent: None,
                slot: None,
                items: Vec::new(),
                opens: Vec::new(),
            }],
            home: HashMap::new(),
        };
        t.add(0, elements);
        t
    }

    /// Clusters are numbered in pre-order, so a child always has a larger
    /// index than its parent.
    fn add(&mut self, c: usize, items: &'a [C4Element]) {
        for el in items {
            let idx = self.clusters[c].items.len();
            self.clusters[c].items.push(el);
            self.home.entry(el.alias.as_str()).or_insert((c, idx));
            if el.boundary_kind.is_some() {
                let cc = self.clusters.len();
                self.clusters.push(Cluster {
                    boundary: Some(el),
                    parent: Some(c),
                    slot: Some(idx),
                    items: Vec::new(),
                    opens: Vec::new(),
                });
                self.clusters[c].opens.push(Some(cc));
                self.add(cc, &el.members);
            } else {
                self.clusters[c].opens.push(None);
            }
        }
    }

    /// Clusters containing `alias`, innermost first, ending at the root.
    fn ancestors(&self, alias: &str) -> Option<Vec<usize>> {
        let mut c = self.home.get(alias)?.0;
        let mut out = vec![c];
        while let Some(p) = self.clusters[c].parent {
            out.push(p);
            c = p;
        }
        Some(out)
    }

    /// Index, within cluster `c`, of the direct item that is or holds `alias`.
    /// Walks parent links (never aliases, which may be duplicated), and a
    /// parent always has a smaller index, so this always terminates.
    pub fn item_in(&self, alias: &str, c: usize) -> Option<usize> {
        let (mut hc, mut idx) = *self.home.get(alias)?;
        while hc != c {
            let cluster = &self.clusters[hc];
            (hc, idx) = (cluster.parent?, cluster.slot?);
        }
        Some(idx)
    }

    /// The cluster a boundary alias opens, if `alias` names a boundary.
    fn opened_by(&self, alias: &str) -> Option<usize> {
        let &(c, i) = self.home.get(alias)?;
        self.clusters[c].opens[i]
    }
}

/// Where a relation is routed: as a plain edge in its endpoints' deepest
/// common cluster, plus a port edge in every cluster between that and each
/// endpoint (innermost first).
pub(super) struct RelPath {
    pub common: usize,
    pub src_side: Vec<usize>,
    pub dst_side: Vec<usize>,
}

pub(super) fn rel_path(tree: &Tree, r: &C4Relation) -> Option<RelPath> {
    if r.from == r.to {
        return None;
    }
    let up_s = tree.ancestors(&r.from)?;
    let up_t = tree.ancestors(&r.to)?;
    // A relation between a boundary and something inside it has no sibling
    // pair to route between; `rel::plan_rels` draws it from the frame edge.
    let encloses = |b: &str, up: &[usize]| tree.opened_by(b).is_some_and(|c| up.contains(&c));
    if encloses(&r.from, &up_t) || encloses(&r.to, &up_s) {
        return None;
    }
    let common = *up_s.iter().find(|c| up_t.contains(c))?;
    let below = |up: &[usize]| up.iter().take_while(|&&c| c != common).copied().collect();
    Some(RelPath {
        common,
        src_side: below(&up_s),
        dst_side: below(&up_t),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{C4BoundaryKind, C4ElementKind, C4RelDirection};

    fn el(alias: &str, members: Option<Vec<C4Element>>) -> C4Element {
        C4Element {
            kind: C4ElementKind::Container,
            alias: alias.into(),
            label: alias.into(),
            descr: None,
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: members.as_ref().map(|_| C4BoundaryKind::Container),
            boundary_type: None,
            members: members.unwrap_or_default(),
        }
    }

    fn rel(from: &str, to: &str) -> C4Relation {
        C4Relation {
            from: from.into(),
            to: to.into(),
            label: String::new(),
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
            reversed: false,
        }
    }

    /// `outer { inner { a } b }`, `c` at the root.
    fn sample() -> Vec<C4Element> {
        vec![
            el(
                "outer",
                Some(vec![el("inner", Some(vec![el("a", None)])), el("b", None)]),
            ),
            el("c", None),
        ]
    }

    #[test]
    fn item_in_finds_the_enclosing_item_at_each_level() {
        let els = sample();
        let t = Tree::build(&els);
        // Clusters: 0 root, 1 outer, 2 inner.
        assert_eq!(t.item_in("a", 2), Some(0));
        assert_eq!(t.item_in("a", 1), Some(0), "inner is outer's item 0");
        assert_eq!(t.item_in("a", 0), Some(0), "outer is root's item 0");
        assert_eq!(t.item_in("b", 1), Some(1));
        assert_eq!(t.item_in("c", 1), None, "c is not inside outer");
        assert_eq!(t.item_in("ghost", 0), None);
    }

    #[test]
    fn duplicate_alias_terminates_and_first_declaration_wins() {
        // #396 B1: a member reusing its boundary's alias used to loop forever.
        let els = vec![el("y", None), el("x", Some(vec![el("x", None)]))];
        let t = Tree::build(&els);
        assert_eq!(t.home.get("x"), Some(&(0, 1)), "the boundary came first");
        assert_eq!(t.item_in("x", 0), Some(1));
        assert_eq!(t.item_in("x", 1), None);
        let p = rel_path(&t, &rel("y", "x")).expect("routed between root items");
        assert_eq!((p.common, p.src_side.len(), p.dst_side.len()), (0, 0, 0));
    }

    #[test]
    fn rel_path_lists_the_clusters_between_endpoints_and_common_level() {
        let els = sample();
        let t = Tree::build(&els);
        let p = rel_path(&t, &rel("a", "c")).expect("path");
        assert_eq!(p.common, 0);
        assert_eq!(p.src_side, vec![2, 1], "innermost first");
        assert!(p.dst_side.is_empty());
        let p = rel_path(&t, &rel("b", "a")).expect("path");
        assert_eq!(
            (p.common, p.src_side.clone(), p.dst_side.clone()),
            (1, vec![], vec![2])
        );
        assert!(rel_path(&t, &rel("a", "a")).is_none(), "self relation");
        assert!(
            rel_path(&t, &rel("outer", "a")).is_none(),
            "boundary to member"
        );
        assert!(
            rel_path(&t, &rel("a", "inner")).is_none(),
            "member to boundary"
        );
        assert!(rel_path(&t, &rel("a", "ghost")).is_none());
    }
}
