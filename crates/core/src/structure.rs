//! Confidence in the organization of a course.
//!
//! It measures how clear the arrangement of a course folder is from the already-derived catalog:
//! whether there are lessons with media, whether the order comes from numbering and whether any material was left loose. It does not
//! read the disk nor write to the database, and does not change the tree; the result lets the user
//! understand why the organization may differ from what was expected.

use crate::catalog::{CatalogFile, CatalogNode, NodeRole};
use crate::scanner::MediaKind;

/// How trustworthy the detected organization is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// No structure concern.
    High,
    /// One concern about order or materials.
    Medium,
    /// Video outside a lesson, no lessons, or more than one concern.
    Low,
}

/// Reason that lowers confidence. Only the facts appear here; the text belongs to the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concern {
    /// No lesson contains a video.
    NoLessons,
    /// There is a video outside any lesson, so it does not appear in the list of lessons.
    LooseMedia,
    /// Most lessons have no number in the name, so the order is alphabetical.
    LessonsWithoutNumbering,
    /// There are materials that belong to no lesson.
    LooseMaterials,
}

/// Assessment of a course: the level and the reasons, in the order they were found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureAssessment {
    /// Resulting confidence level.
    pub confidence: Confidence,
    /// Concerns found, in detection order.
    pub concerns: Vec<Concern>,
}

/// Assesses the organization of a course from its catalog tree.
///
/// A video outside a lesson or a total absence of lessons leads to `Low`. Otherwise each
/// concern about order or materials counts: none is `High`, one is `Medium` and two or
/// more are `Low`.
pub fn assess(course: &CatalogNode) -> StructureAssessment {
    assess_parts(&course.children, &course.loose_files)
}

/// Assesses a course from the modules and lessons and the loose files at the top, without requiring
/// the course node to be assembled. Used by the server, which already has these parts separate.
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

/// Collects the lessons of the tree and flags a video or material outside any lesson.
fn collect<'a>(
    node: &'a CatalogNode,
    lessons: &mut Vec<&'a CatalogNode>,
    loose_media: &mut bool,
    loose_materials: &mut bool,
) {
    if node.role == NodeRole::Lesson {
        lessons.push(node);
    }
    let has_children = !node.children.is_empty();
    for file in &node.loose_files {
        match file.kind {
            MediaKind::Media => *loose_media = true,
            MediaKind::Subtitle => *loose_materials = true,
            MediaKind::Material => {
                // Supplementary handouts inside a structured module that contains child
                // lessons/submodules are expected course materials, not misplaced root files.
                if !(node.role == NodeRole::Category && has_children) {
                    *loose_materials = true;
                }
            }
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

/// True when the last segment of the path starts with a digit, like `01 Intro`.
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

    #[test]
    fn module_level_supplementary_materials_keep_high_confidence() {
        let module = CatalogNode {
            title: "01 Module".to_owned(),
            path: "Cat/Course/01 Module".to_owned(),
            role: NodeRole::Category,
            files: Vec::new(),
            loose_files: vec![file("handout.pdf", MediaKind::Material)],
            children: vec![
                lesson(
                    "Cat/Course/01 Module/01 Intro",
                    vec![file("a.mp4", MediaKind::Media)],
                ),
                lesson(
                    "Cat/Course/01 Module/02 Next",
                    vec![file("b.mp4", MediaKind::Media)],
                ),
            ],
        };
        let tree = course(vec![module], Vec::new());
        let assessment = assess(&tree);
        assert_eq!(assessment.confidence, Confidence::High);
        assert!(assessment.concerns.is_empty());
    }
}
