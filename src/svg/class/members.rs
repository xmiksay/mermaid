//! Class member display: generic `~T~` conversion and UML classifier handling.

use crate::parse::Visibility;

/// A member ready to draw: its display text (visibility marker + generics
/// converted to angle brackets, classifier suffix stripped) plus the styling
/// the classifier implies — `*` (abstract) → italic, `$` (static) → underline.
pub(super) struct MemberDisplay {
    pub(super) text: String,
    pub(super) is_abstract: bool,
    pub(super) is_static: bool,
}

pub(super) fn member_display(m: &crate::parse::ClassMember) -> MemberDisplay {
    let vis = match m.visibility {
        Visibility::Public => "+",
        Visibility::Private => "-",
        Visibility::Protected => "#",
        Visibility::Package => "~",
        Visibility::Default => "",
    };
    let (body, is_abstract, is_static) = split_classifier(m.text.trim());
    MemberDisplay {
        text: format!("{vis}{}", convert_generics(body.trim())),
        is_abstract,
        is_static,
    }
}

/// Strip a trailing UML classifier: `*` marks an abstract member, `$` a static
/// one. Returns the text without the classifier and the two flags.
fn split_classifier(text: &str) -> (&str, bool, bool) {
    match text.chars().last() {
        Some('*') => (&text[..text.len() - 1], true, false),
        Some('$') => (&text[..text.len() - 1], false, true),
        _ => (text, false, false),
    }
}

/// Convert Mermaid generic syntax `~T~` to `<T>`, innermost pair first so that
/// nested generics like `List~List~int~~` become `List<List<int>>` and
/// comma-separated ones like `Map~string, int~` become `Map<string, int>`. A
/// lone unmatched `~` is left untouched.
pub(super) fn convert_generics(s: &str) -> String {
    let mut s = s.to_string();
    loop {
        let tildes: Vec<usize> = s.match_indices('~').map(|(i, _)| i).collect();
        // Innermost pair = the adjacent tilde pair with non-empty content and
        // the largest opening index; replacing it first unwinds nesting.
        let chosen = tildes
            .windows(2)
            .filter(|w| w[1] > w[0] + 1)
            .map(|w| (w[0], w[1]))
            .next_back();
        let Some((a, b)) = chosen else { break };
        // Emit entity codes, not raw `<`/`>`: the inline-HTML label pass would
        // otherwise read `List<int>` as an (unknown, stripped) `<int>` tag. The
        // codes decode back to literal angle brackets at render time.
        s = format!("{}#lt;{}#gt;{}", &s[..a], &s[a + 1..b], &s[b + 1..]);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::super::theme::Theme;
    use super::super::render;
    use super::*;
    use crate::parse::{parse, ClassDiagram, Diagram};

    fn build(s: &str) -> ClassDiagram {
        match parse(s).unwrap() {
            Diagram::Class(c) => c,
            _ => panic!("not class"),
        }
    }

    #[test]
    fn generics_convert_to_angle_brackets() {
        assert_eq!(convert_generics("List~int~"), "List#lt;int#gt;");
        assert_eq!(
            convert_generics("Map~string, int~"),
            "Map#lt;string, int#gt;"
        );
        assert_eq!(
            convert_generics("List~List~int~~"),
            "List#lt;List#lt;int#gt;#gt;"
        );
        // A lone unmatched tilde is left alone.
        assert_eq!(convert_generics("a~b"), "a~b");
    }

    #[test]
    fn generics_render_in_name_and_members() {
        let d = build("classDiagram\nclass List~T~ {\n+items List~int~\n+get() T\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">List&lt;T&gt;<"));
        assert!(svg.contains("items List&lt;int&gt;"));
        assert!(!svg.contains('~'));
    }

    #[test]
    fn abstract_and_static_classifiers_style_members() {
        let d = build("classDiagram\nclass Shape {\n+area() float*\n+count int$\n}\n");
        let svg = render(&d, &Theme::default());
        // Classifier chars are consumed, not rendered.
        assert!(!svg.contains('*'));
        assert!(!svg.contains('$'));
        assert!(svg.contains("font-style=\"italic\""));
        assert!(svg.contains("text-decoration=\"underline\""));
    }
}
