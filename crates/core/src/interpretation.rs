//! Sugestões heurísticas sobre uma árvore descoberta.
//!
//! Nada aqui altera a árvore física ou cria classificações persistidas. Toda sugestão é
//! acompanhada de explicação e pode ser ambígua; a confirmação humana continua sendo o
//! contrato da futura importação.

use serde::Serialize;

use crate::discovery::{DiscoveryEntryKind, DiscoveryNode, DiscoveryReport, DiscoveryStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Nível de confiança de uma sugestão, sem caráter definitivo.
pub enum SuggestionConfidence {
    /// Pista com evidência estrutural forte, ainda não definitiva.
    High,
    /// Pista razoável que precisa de confirmação.
    Medium,
    /// Pista fraca ou ambígua.
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// Sugestão explicável de organização para um caminho físico.
pub struct OrganizationSuggestion {
    /// Caminho relativo ao diretório selecionado.
    pub path: String,
    /// Papel pedagógico sugerido, sem persistência.
    pub proposed_role: String,
    /// Força da evidência usada pela heurística.
    pub confidence: SuggestionConfidence,
    /// Explicação humana da sugestão.
    pub explanation: String,
    /// Indica que a estrutura exige revisão antes de qualquer importação.
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// Resultado não persistido da interpretação heurística.
pub struct InterpretationPreview {
    /// Sugestões não persistidas derivadas da árvore física.
    pub suggestions: Vec<OrganizationSuggestion>,
}

/// Interpreta uma descoberta sem modificar a árvore, o banco ou a classificação futura.
pub fn interpret(report: &DiscoveryReport) -> InterpretationPreview {
    let mut suggestions = Vec::new();
    interpret_node(&report.root, &mut suggestions);
    InterpretationPreview { suggestions }
}

fn interpret_node(node: &DiscoveryNode, suggestions: &mut Vec<OrganizationSuggestion>) {
    if matches!(node.entry_kind, DiscoveryEntryKind::Directory)
        && matches!(node.status, DiscoveryStatus::Found)
    {
        let direct_files = node
            .children
            .iter()
            .filter(|child| {
                matches!(child.entry_kind, DiscoveryEntryKind::File)
                    && matches!(child.status, DiscoveryStatus::Found)
            })
            .count();
        let direct_dirs = node
            .children
            .iter()
            .filter(|child| {
                matches!(child.entry_kind, DiscoveryEntryKind::Directory)
                    && matches!(child.status, DiscoveryStatus::Found)
            })
            .count();
        let media_descendants = count_media(node);
        if direct_files > 0 && direct_dirs == 0 && media_descendants > 0 {
            suggestions.push(OrganizationSuggestion { path: node.path.clone(), proposed_role: "possível aula".to_owned(), confidence: SuggestionConfidence::Medium,
                explanation: "A pasta contém arquivos reproduzíveis diretamente; isso sugere uma aula, mas o nome e a profundidade não provam essa intenção.".to_owned(), ambiguous: false });
        } else if direct_dirs > 0 && media_descendants > 0 {
            let ambiguous = direct_files > 0;
            suggestions.push(OrganizationSuggestion { path: node.path.clone(), proposed_role: "possível módulo ou curso".to_owned(), confidence: if ambiguous { SuggestionConfidence::Low } else { SuggestionConfidence::Medium },
                explanation: if ambiguous { "A pasta mistura arquivos e subpastas; a estrutura física foi preservada e a organização pedagógica precisa de revisão.".to_owned() } else { "A pasta agrupa subpastas com arquivos reproduzíveis; isso é uma pista estrutural, não uma classificação definitiva.".to_owned() }, ambiguous });
        }
        if has_numeric_prefix(&node.name) {
            suggestions.push(OrganizationSuggestion { path: node.path.clone(), proposed_role: "possível ordem natural".to_owned(), confidence: SuggestionConfidence::Low,
                explanation: "O nome começa com numeração; isso pode indicar ordem, mas números em nomes não são prova de sequência pedagógica.".to_owned(), ambiguous: true });
        }
    }
    for child in &node.children {
        interpret_node(child, suggestions);
    }
}

fn count_media(node: &DiscoveryNode) -> usize {
    let own = node
        .children
        .iter()
        .filter(|child| {
            matches!(
                child.file_kind,
                Some(crate::discovery::DiscoveryFileKind::Media)
            ) && matches!(child.status, DiscoveryStatus::Found)
        })
        .count();
    own + node.children.iter().map(count_media).sum::<usize>()
}

fn has_numeric_prefix(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::discover;

    #[test]
    fn marks_mixed_structure_as_ambiguous_without_rewriting_it() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("01 - Módulo/Sub")).unwrap();
        std::fs::write(temp.path().join("01 - Módulo/direct.mp4"), b"x").unwrap();
        std::fs::write(temp.path().join("01 - Módulo/Sub/next.mp4"), b"x").unwrap();
        let report = discover(temp.path()).unwrap();
        let preview = interpret(&report);
        assert!(
            preview
                .suggestions
                .iter()
                .any(|item| item.path == "01 - Módulo" && item.ambiguous)
        );
        assert_eq!(report.root.children[0].children.len(), 2);
    }
}
