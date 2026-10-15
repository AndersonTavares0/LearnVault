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
//! a categoria. Nenhum arquivo é descartado por estar em outra profundidade — o que não
//! encontrar um nó correspondente é preservado em `Catalog::loose_files` (raiz da
//! biblioteca) ou em `CatalogNode::loose_files` (arquivo direto na categoria).
//!
//! Cada nó é identificado pelo **caminho relativo completo**, nunca pelo título: dois
//! cursos com o mesmo nome em caminhos diferentes continuam sendo nós distintos.

use std::collections::BTreeMap;

use crate::db::Database;
use crate::error::Result;
use crate::paths::{KeyMode, relative_path_key};
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
    /// Arquivos da aula. Só a aula preenche este campo.
    pub files: Vec<CatalogFile>,
    /// Arquivos que estão diretamente neste nó sem pertencer a uma aula. Só a
    /// categoria preenche este campo.
    pub loose_files: Vec<CatalogFile>,
    /// Nós filhos, ordenados por caminho.
    pub children: Vec<CatalogNode>,
}

/// Catálogo derivado de uma biblioteca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// Verdadeiro quando a última varredura cobriu toda a biblioteca. Sem varredura
    /// registrada, não há nada pendente e o valor é verdadeiro.
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
    let (complete, unverified) = last_scan(db, library_id, mode)?;
    let rows = media_rows(db, library_id)?;

    let mut loose_files = Vec::new();
    let mut nodes: BTreeMap<String, PendingNode> = BTreeMap::new();

    for row in rows {
        let size = u64::try_from(row.size).unwrap_or(0);
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
            availability: availability_of(&row.availability),
            unverified: is_under_unverified(&row.relative_path_key, &unverified),
        };

        let mut segments: Vec<&str> = row.relative_path.split('/').collect();
        // O último segmento é o nome do arquivo; o diretório é o restante.
        segments.pop();

        match segments.len() {
            0 => loose_files.push(file),
            1 => {
                let category = ensure_node(&mut nodes, segments[0], NodeRole::Category);
                category.loose_files.push(file);
            }
            depth => {
                let category_path = segments[0].to_owned();
                if depth >= 4 {
                    let course_path = segments[..depth - 2].join("/");
                    ensure_node(&mut nodes, &course_path, NodeRole::Course);
                }
                if depth >= 3 {
                    let module_path = segments[..depth - 1].join("/");
                    ensure_node(&mut nodes, &module_path, NodeRole::Module);
                }
                ensure_node(&mut nodes, &category_path, NodeRole::Category);
                let lesson_path = segments.join("/");
                let lesson = ensure_node(&mut nodes, &lesson_path, NodeRole::Lesson);
                lesson.files.push(file);
            }
        }
    }

    let categories = assemble(&mut nodes);
    Ok(Catalog {
        complete,
        loose_files,
        categories,
    })
}

/// Nó em construção, antes de a árvore ser montada.
struct PendingNode {
    title: String,
    role: NodeRole,
    files: Vec<CatalogFile>,
    loose_files: Vec<CatalogFile>,
}

/// Devolve o nó do caminho indicado, criando-o com o papel dado se ainda não existir.
///
/// O primeiro nó a reivindicar um caminho define o papel: um diretório que contém
/// arquivos diretamente é uma aula, e continua sendo mesmo que um arquivo mais profundo
/// tente descrevê-lo como módulo ou curso.
fn ensure_node<'a>(
    nodes: &'a mut BTreeMap<String, PendingNode>,
    path: &str,
    role: NodeRole,
) -> &'a mut PendingNode {
    nodes.entry(path.to_owned()).or_insert_with(|| PendingNode {
        title: clean_title(last_segment(path)),
        role,
        files: Vec::new(),
        loose_files: Vec::new(),
    })
}

/// Monta a árvore a partir dos nós pendentes, ligando cada nó ao ancestral existente
/// mais profundo.
fn assemble(nodes: &mut BTreeMap<String, PendingNode>) -> Vec<CatalogNode> {
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut roots = Vec::new();
    for path in nodes.keys() {
        match deepest_existing_ancestor(path, nodes) {
            Some(parent) => children.entry(parent).or_default().push(path.clone()),
            None => roots.push(path.clone()),
        }
    }
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
    let nested = children
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
        role: pending.role,
        files: pending.files,
        loose_files: pending.loose_files,
        children: nested,
    })
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
fn clean_title(name: &str) -> String {
    name.trim_start_matches(|character: char| "0123456789._- •\t".contains(character))
        .trim()
        .to_owned()
}

/// Converte o estado gravado em `media.availability`.
fn availability_of(stored: &str) -> Availability {
    if stored == "missing" {
        Availability::Missing
    } else {
        Availability::Available
    }
}

/// Indica se uma chave de caminho está sob alguma área não verificada.
fn is_under_unverified(key: &str, areas: &[String]) -> bool {
    areas.iter().any(|area| {
        key == area
            || (key.len() > area.len()
                && key.starts_with(area.as_str())
                && key.as_bytes()[area.len()] == b'/')
    })
}

/// Linha de `media` necessária para montar o catálogo.
struct MediaRow {
    media_id: String,
    relative_path: String,
    relative_path_key: String,
    size: i64,
    availability: String,
}

/// Lê a última varredura da biblioteca e as áreas que ela não conseguiu verificar.
fn last_scan(db: &Database, library_id: &str, mode: KeyMode) -> Result<(bool, Vec<String>)> {
    let connection = db.connection();
    let (scan_id, complete) = {
        let mut statement = connection.prepare(
            "SELECT scan_id, complete FROM scan WHERE library_id = ?1 ORDER BY scan_id DESC LIMIT 1",
        )?;
        let mut rows = statement.query([library_id])?;
        match rows.next()? {
            Some(row) => (row.get::<_, i64>(0)?, row.get::<_, i64>(1)?),
            None => return Ok((true, Vec::new())),
        }
    };

    let mut statement = connection.prepare(
        "SELECT relative_path FROM scan_inaccessible WHERE scan_id = ?1 ORDER BY relative_path",
    )?;
    let stored = statement
        .query_map([scan_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    let mut unverified = Vec::new();
    for path in stored {
        if let Ok(key) = relative_path_key(&path, mode) {
            unverified.push(key);
        }
    }
    Ok((complete != 0, unverified))
}

/// Lê as linhas de mídia da biblioteca, já ordenadas por chave de caminho.
fn media_rows(db: &Database, library_id: &str) -> Result<Vec<MediaRow>> {
    let mut statement = db.connection().prepare(
        "SELECT media_id, relative_path, relative_path_key, size, availability
         FROM media
         WHERE library_id = ?1
         ORDER BY relative_path_key",
    )?;
    let rows = statement.query_map([library_id], |row| {
        Ok(MediaRow {
            media_id: row.get(0)?,
            relative_path: row.get(1)?,
            relative_path_key: row.get(2)?,
            size: row.get(3)?,
            availability: row.get(4)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
