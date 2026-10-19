//! Derivação do catálogo a partir dos dados persistidos.
//!
//! O catálogo é uma função do que está gravado: ele lê `media`, a última linha de `scan`
//! e as áreas de `scan_inaccessible`. Nunca acessa o sistema de arquivos e nunca escreve
//! no banco. A varredura (1.3) descobre os arquivos e a política de rescan (1.4) decide a
//! disponibilidade; esta camada apenas organiza o que já está registrado.
//!
//! A hierarquia é derivada de baixo para cima a partir dos caminhos persistidos:
//!
//! - **aula**: diretório que contém arquivos diretamente;
//! - **módulo**: diretório-pai da aula;
//! - **curso**: diretório-pai do módulo;
//! - **categoria**: primeiro segmento do caminho.
//!
//! Níveis que não existem na biblioteca são omitidos, em vez de sintetizar nós
//! artificiais: em `Categoria/Curso/aula.mp4` não há módulo e a aula fica diretamente sob
//! a categoria.
//!
//! O papel de cada nó é decidido pela estrutura final, nunca pela ordem em que os arquivos
//! foram lidos: um nó que recebe arquivos é aula, o nó que organiza aulas é módulo e o que
//! organiza módulos é curso. Assim o papel de um diretório não muda por causa do nome de um
//! arquivo vizinho.
//!
//! Um diretório com mídia dentro de uma aula — por exemplo `Aula 1/extras/mais.mp4` — é
//! outra aula, aninhada sob a primeira. Caminhos mais profundos que o modelo de quatro
//! níveis não criam níveis extras: os segmentos entre a categoria e o curso ficam
//! incorporados no caminho do nó de curso.
//!
//! Nenhum arquivo é descartado por estar em outra profundidade — o que não encontrar um nó
//! correspondente é preservado em `Catalog::loose_files` (raiz da biblioteca) ou em
//! `CatalogNode::loose_files` (arquivo direto na categoria).
//!
//! Quando a última tentativa de varredura falhou, nada pôde ser verificado: o catálogo é
//! marcado como incompleto e todos os arquivos ficam sinalizados como não verificados.
//!
//! Cada nó é identificado pelo **caminho relativo completo**, nunca pelo título: dois
//! cursos com o mesmo nome em caminhos diferentes continuam sendo nós distintos.

use std::collections::BTreeMap;

use crate::db::Database;
use crate::error::Result;
use crate::paths::{KeyMode, is_under_area, relative_path_key};
use crate::scanner::{MediaKind, classify};

/// Disponibilidade de um arquivo segundo a última varredura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// O arquivo foi encontrado na última varredura.
    Available,
    /// O arquivo era conhecido e não foi encontrado. O registro e a identidade
    /// continuam válidos.
    Missing,
}

/// Papel de um nó na hierarquia do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Primeiro nível sob a raiz da biblioteca.
    Category,
    /// Conjunto de módulos.
    Course,
    /// Conjunto de aulas.
    Module,
    /// Diretório que contém os arquivos.
    Lesson,
}

/// Arquivo do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFile {
    /// Identidade estável do arquivo, independente da localização.
    pub media_id: String,
    /// Nome do arquivo, sem diretórios.
    pub name: String,
    /// Caminho relativo completo, usado como referência estrutural.
    pub path: String,
    /// Tamanho em bytes na última observação.
    pub size: u64,
    /// Modificação da mídia em segundos desde a época Unix na última observação.
    pub mtime_seconds: i64,
    /// Tipo derivado do nome e do tamanho.
    pub kind: MediaKind,
    /// Disponibilidade na última varredura.
    pub availability: Availability,
    /// Verdadeiro quando o arquivo está sob uma área que a última varredura não
    /// conseguiu verificar.
    pub unverified: bool,
}

/// Nó da árvore do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogNode {
    /// Título apresentado, com prefixos numéricos e separadores removidos.
    pub title: String,
    /// Caminho relativo completo que identifica o nó.
    pub path: String,
    /// Papel do nó na hierarquia.
    pub role: NodeRole,
    /// Arquivos do diretório. Só os nós com papel `Lesson` preenchem este campo.
    pub files: Vec<CatalogFile>,
    /// Arquivos sem aula própria, como materiais e legendas sem mídia correspondente.
    /// Categorias e módulos podem preenchê-lo.
    pub loose_files: Vec<CatalogFile>,
    /// Nós filhos, ordenados por caminho.
    pub children: Vec<CatalogNode>,
}

/// Catálogo derivado de uma biblioteca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// Verdadeiro quando a última varredura cobriu toda a biblioteca. Sem varredura
    /// registrada, não há nada pendente e o valor é verdadeiro. Uma tentativa que falhou
    /// deixa o valor falso.
    pub complete: bool,
    /// Arquivos na raiz da biblioteca, que não pertencem a nenhuma categoria.
    pub loose_files: Vec<CatalogFile>,
    /// Categorias, ordenadas por caminho.
    pub categories: Vec<CatalogNode>,
}

/// Constrói o catálogo de uma biblioteca a partir dos dados persistidos.
///
/// Não recebe a raiz da biblioteca de propósito: sem um caminho, esta função não tem como
/// ler o disco. Ela também não escreve no banco.
pub fn build_catalog(db: &Database, library_id: &str) -> Result<Catalog> {
    let mode = KeyMode::for_host();
    let scan = last_scan(db, library_id, mode)?;
    let rows = media_rows(db, library_id)?;

    let mut loose_files = Vec::new();
    let mut nodes: BTreeMap<String, PendingNode> = BTreeMap::new();

    for row in rows {
        let size = u64::try_from(row.size).map_err(|_| crate::error::Error::MediaCorruptSize {
            media_id: row.media_id.clone(),
        })?;
        let name = row
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&row.relative_path)
            .to_owned();
        let file = CatalogFile {
            kind: classify(&name, size),
            media_id: row.media_id,
            name,
            path: row.relative_path.clone(),
            size,
            mtime_seconds: row.mtime,
            availability: availability_of(&row.availability),
            unverified: scan.failed || is_under_unverified(&row.relative_path_key, &scan.areas),
        };

        let mut segments: Vec<&str> = row.relative_path.split('/').collect();
        // O último segmento é o nome do arquivo; o diretório é o restante.
        segments.pop();

        match segments.len() {
            0 => loose_files.push(file),
            1 => {
                let category = ensure_node(&mut nodes, segments[0]);
                category.loose_files.push(file);
            }
            depth => {
                if depth >= 4 {
                    ensure_node(&mut nodes, &segments[..depth - 2].join("/"));
                }
                if depth >= 3 {
                    ensure_node(&mut nodes, &segments[..depth - 1].join("/"));
                }
                ensure_node(&mut nodes, segments[0]);
                let lesson = ensure_node(&mut nodes, &segments.join("/"));
                lesson.files.push(file);
            }
        }
    }

    loose_files.sort_by(|a, b| natural_cmp(a.path.as_str(), b.path.as_str()));
    let categories = assemble(&mut nodes);
    Ok(Catalog {
        complete: scan.complete,
        loose_files,
        categories,
    })
}

/// Nó em construção, antes de a árvore ser montada.
struct PendingNode {
    title: String,
    files: Vec<CatalogFile>,
    loose_files: Vec<CatalogFile>,
}

/// Devolve o nó do caminho indicado, criando-o se ainda não existir.
///
/// O papel do nó não é decidido aqui: ele depende da estrutura final e é calculado
/// quando a árvore é montada, em [`role_of`].
fn ensure_node<'a>(
    nodes: &'a mut BTreeMap<String, PendingNode>,
    path: &str,
) -> &'a mut PendingNode {
    nodes.entry(path.to_owned()).or_insert_with(|| PendingNode {
        title: clean_title(last_segment(path)),
        files: Vec::new(),
        loose_files: Vec::new(),
    })
}

/// Monta a árvore a partir dos nós pendentes, ligando cada nó ao ancestral existente
/// mais profundo.
fn assemble(nodes: &mut BTreeMap<String, PendingNode>) -> Vec<CatalogNode> {
    let children: BTreeMap<String, Vec<String>> = {
        let keys: Vec<&str> = nodes.keys().map(String::as_str).collect();
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for path in &keys {
            if let Some(parent) = deepest_existing_ancestor(path, nodes) {
                children
                    .entry(parent.to_owned())
                    .or_default()
                    .push((*path).to_owned());
            }
        }
        for list in children.values_mut() {
            list.sort_by(|a, b| natural_cmp(a, b));
        }
        children
    };
    let mut roots: Vec<String> = nodes
        .keys()
        .filter_map(|path| {
            if deepest_existing_ancestor(path, nodes).is_none() {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();
    roots.sort_by(|a, b| natural_cmp(a, b));
    roots
        .iter()
        .filter_map(|path| take_node(path, nodes, &children))
        .collect()
}

/// Remove o nó pendente e devolve-o já com os filhos montados.
fn take_node(
    path: &str,
    nodes: &mut BTreeMap<String, PendingNode>,
    children: &BTreeMap<String, Vec<String>>,
) -> Option<CatalogNode> {
    let pending = nodes.remove(path)?;
    let natural: fn(&CatalogFile, &CatalogFile) -> std::cmp::Ordering =
        |file_a, file_b| natural_cmp(file_a.path.as_str(), file_b.path.as_str());
    let mut files = pending.files;
    files.sort_by(natural);
    let mut loose_files = pending.loose_files;
    loose_files.sort_by(natural);
    let nested: Vec<CatalogNode> = children
        .get(path)
        .map(|list| {
            list.iter()
                .filter_map(|child| take_node(child, nodes, children))
                .collect()
        })
        .unwrap_or_default();
    Some(CatalogNode {
        title: pending.title,
        path: path.to_owned(),
        role: role_of(path, &files, &nested),
        files,
        loose_files,
        children: nested,
    })
}

/// Papel de um nó, decidido pela estrutura final e nunca pela ordem de leitura.
///
/// O primeiro nível é sempre a categoria. Abaixo dele, um nó que recebeu arquivos é uma
/// aula, o nó que organiza aulas é um módulo e o que organiza módulos é um curso.
fn role_of(path: &str, files: &[CatalogFile], children: &[CatalogNode]) -> NodeRole {
    if !path.contains('/') {
        return NodeRole::Category;
    }
    if !files.is_empty() {
        return NodeRole::Lesson;
    }
    if children.iter().any(|child| child.role == NodeRole::Module) {
        return NodeRole::Course;
    }
    NodeRole::Module
}

/// Compara textos com ordenação natural: sequências de dígitos são lidas como número
/// (com desempate pela notação, para total determinismo) e o restante é comparado
/// caractere a caractere. "2" vem antes de "10" em qualquer nível da árvore.
pub fn natural_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (left, right);
    loop {
        match (natural_segment(a), natural_segment(b)) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some((segment_a, rest_a)), Some((segment_b, rest_b))) => {
                let ordering = compare_natural_segment(segment_a, segment_b);
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
                a = rest_a;
                b = rest_b;
            }
        }
    }
}

/// Primeira sequência homogênea da string (só dígitos ou nenhum dígito) e o restante.
fn natural_segment(input: &str) -> Option<(&str, &str)> {
    if input.is_empty() {
        return None;
    }
    let digit_run = input.as_bytes()[0].is_ascii_digit();
    let end = input
        .char_indices()
        .find(|(_, character)| character.is_ascii_digit() != digit_run)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    Some((&input[..end], &input[end..]))
}

/// Compara duas sequências homogêneas, lendo pares de dígitos como número.
fn compare_natural_segment(a: &str, b: &str) -> std::cmp::Ordering {
    if a.as_bytes()[0].is_ascii_digit() && b.as_bytes()[0].is_ascii_digit() {
        let number_a = a.trim_start_matches('0');
        let number_b = b.trim_start_matches('0');
        return number_a
            .len()
            .cmp(&number_b.len())
            .then_with(|| number_a.cmp(number_b))
            .then_with(|| a.cmp(b));
    }
    a.cmp(b)
}

/// Ancestral existente mais profundo de um caminho, se houver.
fn deepest_existing_ancestor(path: &str, nodes: &BTreeMap<String, PendingNode>) -> Option<String> {
    let mut best: Option<&str> = None;
    for candidate in nodes.keys() {
        let is_ancestor = candidate.len() < path.len()
            && path.starts_with(candidate.as_str())
            && path.as_bytes()[candidate.len()] == b'/';
        if is_ancestor && best.is_none_or(|current| candidate.len() > current.len()) {
            best = Some(candidate);
        }
    }
    best.map(str::to_owned)
}

/// Último segmento de um caminho relativo.
fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Título apresentado de um nó: sem prefixos numéricos nem separadores de ordenação.
///
/// Compartilhada com o registro de cursos, que deriva o título do card do nome da pasta,
/// e com a aula de arquivo, que limpa o nome do arquivo antes de estripar a extensão.
pub fn clean_title(name: &str) -> String {
    name.trim_start_matches(|character: char| "0123456789._- •\t".contains(character))
        .trim()
        .to_owned()
}

/// Título de aula derivado de um arquivo: sem extensão e sem prefixo de ordenação.
fn file_lesson_title(name: &str) -> String {
    let stem = match name.rfind('.') {
        Some(index) if index > 0 => &name[..index],
        _ => name,
    };
    clean_title(stem)
}

/// Recompõe cursos e módulos: arquivos de mídia diretamente no diretório viram aulas,
/// acompanhados por legendas com o mesmo nome-base. Materiais e legendas sem mídia
/// correspondente continuam disponíveis em `loose_files`.
///
/// A transformação é recursiva e não altera aulas que o indexador já derivou de
/// diretórios, inclusive quando o módulo também contém arquivos diretamente.
pub fn course_file_lessons(node: CatalogNode) -> CatalogNode {
    let CatalogNode {
        title,
        path,
        role,
        files,
        loose_files,
        children,
    } = node;
    let mut children: Vec<_> = children.into_iter().map(course_file_lessons).collect();
    let mut media = Vec::new();
    let mut loose = Vec::new();
    for file in loose_files {
        if file.kind == MediaKind::Media {
            media.push(file);
        } else {
            loose.push(file);
        }
    }

    if media.is_empty() {
        return CatalogNode {
            title,
            path,
            role,
            files,
            loose_files: loose,
            children,
        };
    }

    for file in media {
        let stem = file_stem(&file.name);
        let mut lesson_files = Vec::new();
        let mut index = 0;
        while index < loose.len() {
            if loose[index].kind == MediaKind::Subtitle && file_stem(&loose[index].name) == stem {
                lesson_files.push(loose.remove(index));
            } else {
                index += 1;
            }
        }
        lesson_files.insert(0, file);
        let media_file = &lesson_files[0];
        children.push(CatalogNode {
            title: file_lesson_title(&media_file.name),
            path: media_file.path.clone(),
            role: NodeRole::Lesson,
            files: lesson_files,
            loose_files: Vec::new(),
            children: Vec::new(),
        });
    }
    children.sort_by(|left, right| natural_cmp(&left.path, &right.path));
    CatalogNode {
        title,
        path,
        role: NodeRole::Module,
        files,
        loose_files: loose,
        children,
    }
}

fn file_stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Converte o estado gravado em `media.availability`.
///
/// É compartilhada com a remoção explícita, que relata a disponibilidade no impacto.
pub(crate) fn availability_of(stored: &str) -> Availability {
    if stored == "missing" {
        Availability::Missing
    } else {
        Availability::Available
    }
}

/// Indica se uma chave de caminho está sob alguma área não verificada.
fn is_under_unverified(key: &str, areas: &[String]) -> bool {
    areas.iter().any(|area| is_under_area(key, area))
}

/// Linha de `media` necessária para montar o catálogo.
struct MediaRow {
    media_id: String,
    relative_path: String,
    relative_path_key: String,
    size: i64,
    mtime: i64,
    availability: String,
}

/// Estado da última varredura registrada para a biblioteca.
struct LastScan {
    /// Verdadeiro quando a varredura cobriu toda a biblioteca.
    complete: bool,
    /// Verdadeiro quando a última tentativa falhou: nada pôde ser verificado.
    failed: bool,
    /// Áreas que a varredura não conseguiu verificar, já normalizadas.
    areas: Vec<String>,
}

/// Lê a última varredura da biblioteca e as áreas que ela não conseguiu verificar.
fn last_scan(db: &Database, library_id: &str, mode: KeyMode) -> Result<LastScan> {
    let connection = db.connection();
    let (scan_id, complete, status) = {
        let mut statement = connection.prepare(
            "SELECT scan_id, complete, status FROM scan WHERE library_id = ?1 ORDER BY scan_id DESC LIMIT 1",
        )?;
        let mut rows = statement.query([library_id])?;
        match rows.next()? {
            Some(row) => (
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ),
            None => {
                return Ok(LastScan {
                    complete: true,
                    failed: false,
                    areas: Vec::new(),
                });
            }
        }
    };

    let mut statement = connection.prepare(
        "SELECT relative_path FROM scan_inaccessible WHERE scan_id = ?1 ORDER BY relative_path",
    )?;
    let stored = statement
        .query_map([scan_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    let mut areas = Vec::new();
    for path in stored {
        if let Ok(key) = relative_path_key(&path, mode) {
            areas.push(key);
        }
    }
    Ok(LastScan {
        complete: complete != 0,
        failed: status == "failed",
        areas,
    })
}

/// Lê as linhas de mídia da biblioteca, já ordenadas por chave de caminho.
///
/// As mídias removidas explicitamente ficam de fora: a remoção tira o item do catálogo
/// sem apagar a linha, e por isso é filtrada aqui, na leitura, em vez de ser um estado da
/// própria linha.
fn media_rows(db: &Database, library_id: &str) -> Result<Vec<MediaRow>> {
    let mut statement = db.connection().prepare(
        "SELECT media_id, relative_path, relative_path_key, size, mtime, availability
         FROM media
         WHERE library_id = ?1
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                            WHERE r.library_id = media.library_id
                              AND r.media_id = media.media_id)
         ORDER BY relative_path_key",
    )?;
    let rows = statement.query_map([library_id], |row| {
        Ok(MediaRow {
            media_id: row.get(0)?,
            relative_path: row.get(1)?,
            relative_path_key: row.get(2)?,
            size: row.get(3)?,
            mtime: row.get(4)?,
            availability: row.get(5)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
