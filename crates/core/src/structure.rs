//! Confiança na organização de um curso.
//!
//! Mede quão claro é o arranjo de uma pasta de curso a partir do catálogo já derivado:
//! se há aulas com mídia, se a ordem vem de numeração e se algum material ficou solto. Não
//! lê o disco nem escreve no banco, e não altera a árvore; o resultado serve para o usuário
//! entender por que a organização pode estar diferente do esperado.

use crate::catalog::{CatalogFile, CatalogNode, NodeRole};
use crate::scanner::MediaKind;

/// Quão confiável é a organização detectada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// Nenhuma preocupação de estrutura.
    High,
    /// Uma preocupação de ordem ou de materiais.
    Medium,
    /// Vídeo fora de aula, ausência de aulas ou mais de uma preocupação.
    Low,
}

/// Motivo que reduz a confiança. Só os fatos aparecem aqui; o texto é da interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concern {
    /// Nenhuma aula contém um vídeo.
    NoLessons,
    /// Há vídeo fora de qualquer aula, então ele não aparece na lista de aulas.
    LooseMedia,
    /// A maioria das aulas não tem número no nome, então a ordem é alfabética.
    LessonsWithoutNumbering,
    /// Há materiais que não pertencem a nenhuma aula.
    LooseMaterials,
}

/// Avaliação de um curso: o nível e os motivos, na ordem em que foram encontrados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureAssessment {
    /// Nível de confiança resultante.
    pub confidence: Confidence,
    /// Preocupações encontradas, na ordem de detecção.
    pub concerns: Vec<Concern>,
}

/// Avalia a organização de um curso a partir da sua árvore de catálogo.
///
/// Vídeo fora de aula ou ausência total de aulas leva a `Low`. Caso contrário, cada
/// preocupação de ordem ou de materiais conta: nenhuma é `High`, uma é `Medium` e duas ou
/// mais são `Low`.
pub fn assess(course: &CatalogNode) -> StructureAssessment {
    assess_parts(&course.children, &course.loose_files)
}

/// Avalia um curso a partir dos módulos e aulas e dos arquivos soltos no topo, sem exigir
/// o nó de curso montado. Usado pelo servidor, que já tem essas partes separadas.
pub fn assess_parts(children: &[CatalogNode], loose_files: &[CatalogFile]) -> StructureAssessment {
    let mut lessons = Vec::new();
    let mut loose_media = false;
    let mut loose_materials = false;
    for file in loose_files {
        match file.kind {
            MediaKind::Media => loose_media = true,
            MediaKind::Subtitle | MediaKind::Material => loose_materials = true,
        }
    }
    for child in children {
        collect(child, &mut lessons, &mut loose_media, &mut loose_materials);
    }

    let with_media = lessons.iter().filter(|lesson| has_media(lesson)).count();
    let numbered = lessons
        .iter()
        .filter(|lesson| starts_with_number(&lesson.path))
        .count();

    let mut concerns = Vec::new();
    if with_media == 0 {
        concerns.push(Concern::NoLessons);
    }
    if loose_media {
        concerns.push(Concern::LooseMedia);
    }
    if lessons.len() >= 2 && numbered * 2 < lessons.len() {
        concerns.push(Concern::LessonsWithoutNumbering);
    }
    if loose_materials {
        concerns.push(Concern::LooseMaterials);
    }

    let confidence =
        if concerns.contains(&Concern::NoLessons) || concerns.contains(&Concern::LooseMedia) {
            Confidence::Low
        } else {
            match concerns.len() {
                0 => Confidence::High,
                1 => Confidence::Medium,
                _ => Confidence::Low,
            }
        };

    StructureAssessment {
        confidence,
        concerns,
    }
}

/// Coleta as aulas da árvore e marca vídeo ou material fora de qualquer aula.
fn collect<'a>(
    node: &'a CatalogNode,
    lessons: &mut Vec<&'a CatalogNode>,
    loose_media: &mut bool,
    loose_materials: &mut bool,
) {
    if node.role == NodeRole::Lesson {
        lessons.push(node);
    }
    for file in &node.loose_files {
        match file.kind {
            MediaKind::Media => *loose_media = true,
            MediaKind::Subtitle | MediaKind::Material => *loose_materials = true,
        }
    }
    for child in &node.children {
        collect(child, lessons, loose_media, loose_materials);
    }
}

fn has_media(lesson: &CatalogNode) -> bool {
    lesson
        .files
        .iter()
        .any(|file| file.kind == MediaKind::Media)
}

/// Verdadeiro quando o último segmento do caminho começa com dígito, como `01 Intro`.
fn starts_with_number(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .and_then(|name| name.chars().next())
        .is_some_and(|character| character.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{Concern, Confidence, assess};
    use crate::catalog::{Availability, CatalogFile, CatalogNode, NodeRole};
    use crate::scanner::MediaKind;

    fn file(name: &str, kind: MediaKind) -> CatalogFile {
        CatalogFile {
            media_id: format!("id-{name}"),
            name: name.to_owned(),
            path: name.to_owned(),
            size: 1,
            mtime_seconds: 0,
            kind,
            availability: Availability::Available,
            unverified: false,
        }
    }

    fn lesson(path: &str, files: Vec<CatalogFile>) -> CatalogNode {
        CatalogNode {
            title: path.to_owned(),
            path: path.to_owned(),
            role: NodeRole::Lesson,
            files,
            loose_files: Vec::new(),
            children: Vec::new(),
        }
    }

    fn course(children: Vec<CatalogNode>, loose_files: Vec<CatalogFile>) -> CatalogNode {
        CatalogNode {
            title: "Course".to_owned(),
            path: "Cat/Course".to_owned(),
            role: NodeRole::Course,
            files: Vec::new(),
            loose_files,
            children,
        }
    }

    #[test]
    fn numbered_lessons_with_media_are_high_confidence() {
        let tree = course(
            vec![
                lesson("Cat/Course/01 Intro", vec![file("a.mp4", MediaKind::Media)]),
                lesson("Cat/Course/02 Next", vec![file("b.mp4", MediaKind::Media)]),
            ],
            Vec::new(),
        );
        let assessment = assess(&tree);
        assert_eq!(assessment.confidence, Confidence::High);
        assert!(assessment.concerns.is_empty());
    }

    #[test]
    fn a_course_without_videos_is_low_confidence() {
        let tree = course(
            vec![lesson(
                "Cat/Course/Notes",
                vec![file("n.pdf", MediaKind::Material)],
            )],
            Vec::new(),
        );
        let assessment = assess(&tree);
        assert_eq!(assessment.confidence, Confidence::Low);
        assert!(assessment.concerns.contains(&Concern::NoLessons));
    }

    #[test]
    fn loose_video_is_low_confidence() {
        let tree = course(
            vec![lesson(
                "Cat/Course/01 Intro",
                vec![file("a.mp4", MediaKind::Media)],
            )],
            vec![file("extra.mp4", MediaKind::Media)],
        );
        let assessment = assess(&tree);
        assert_eq!(assessment.confidence, Confidence::Low);
        assert!(assessment.concerns.contains(&Concern::LooseMedia));
    }

    #[test]
    fn mostly_unnumbered_lessons_are_medium_confidence() {
        let tree = course(
            vec![
                lesson("Cat/Course/Intro", vec![file("a.mp4", MediaKind::Media)]),
                lesson("Cat/Course/Next", vec![file("b.mp4", MediaKind::Media)]),
                lesson("Cat/Course/03 Last", vec![file("c.mp4", MediaKind::Media)]),
            ],
            Vec::new(),
        );
        let assessment = assess(&tree);
        assert_eq!(assessment.confidence, Confidence::Medium);
        assert_eq!(assessment.concerns, vec![Concern::LessonsWithoutNumbering]);
    }

    #[test]
    fn two_independent_concerns_are_low_confidence() {
        let tree = course(
            vec![
                lesson("Cat/Course/Intro", vec![file("a.mp4", MediaKind::Media)]),
                lesson("Cat/Course/Next", vec![file("b.mp4", MediaKind::Media)]),
            ],
            vec![file("guide.pdf", MediaKind::Material)],
        );
        let assessment = assess(&tree);
        assert_eq!(assessment.confidence, Confidence::Low);
        assert_eq!(
            assessment.concerns,
            vec![Concern::LessonsWithoutNumbering, Concern::LooseMaterials]
        );
    }
}
