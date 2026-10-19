//! Ordenação manual de módulos e aulas sem alterar os cursos.
//!
//! A ordem automática do catálogo é a ordem natural determinística de
//! [`crate::catalog::natural_cmp`]: sequências de dígitos são comparadas como número
//! (`Aula 2` antes de `Aula 10`) e empates equivalentes (`Aula 2`/`Aula 02`) são
//! desempatados pelo nome original, nunca pelo locale do sistema.
//!
//! Este módulo apenas **sobrepõe**, por nó pai, a ordem escolhida pelo usuário. A
//! preferência é uma lista de caminhos relativos já catalogados; ela nunca lê nem
//! escreve nos arquivos do curso e nunca muda a identidade (`media_id`) de uma mídia.
//!
//! A chave `""` (caminho vazio) representa a lista de topo recebida por
//! [`apply_order`]: no adaptador, essa é a lista de módulos diretamente sob o curso. Um
//! filho citado que não existe mais na árvore é simplesmente ignorado; os filhos não
//! citados mantêm a ordem automática, então uma reindexação nunca perde uma aula.

use std::collections::BTreeMap;

use crate::catalog::{CatalogNode, natural_cmp};

/// Preferência de ordenação manual de um único curso, entre irmãos.
///
/// Cada entrada mapeia o caminho de um nó pai para a ordem desejada dos seus filhos.
/// A lista de topo passada a [`apply_order`] corresponde ao pai de caminho vazio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogOrder {
    parents: BTreeMap<String, Vec<String>>,
}

impl CatalogOrder {
    /// Cria uma preferência vazia (ordem automática em todos os pais).
    pub fn new() -> Self {
        Self::default()
    }

    /// Verdadeiro quando não há nenhuma preferência registrada.
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }

    /// Registra a ordem desejada dos filhos de `parent`.
    ///
    /// Uma lista vazia remove a preferência daquele pai (volta à ordem automática).
    pub fn set_children(&mut self, parent: &str, children: Vec<String>) {
        if children.is_empty() {
            self.parents.remove(parent);
        } else {
            self.parents.insert(parent.to_owned(), children);
        }
    }

    /// Ordem desejada dos filhos de `parent`, quando registrada.
    pub fn children(&self, parent: &str) -> Option<&[String]> {
        self.parents.get(parent).map(Vec::as_slice)
    }

    /// Remove a preferência de um pai. Verdadeiro quando havia uma preferência.
    pub fn remove(&mut self, parent: &str) -> bool {
        self.parents.remove(parent).is_some()
    }

    /// Remove todas as preferências, restaurando a ordem automática.
    pub fn clear(&mut self) {
        self.parents.clear();
    }

    /// Substitui o conteúdo pela representação persistida (`pai` -> filhos).
    pub fn from_map(parents: BTreeMap<String, Vec<String>>) -> Self {
        let mut order = Self::default();
        for (parent, children) in parents {
            order.set_children(&parent, children);
        }
        order
    }

    /// Representação persistível (`pai` -> filhos), pronta para serializar.
    pub fn as_map(&self) -> &BTreeMap<String, Vec<String>> {
        &self.parents
    }
}

/// Reordena uma lista de irmãos pela preferência, sem perder nem duplicar ninguém.
///
/// Os caminhos citados vêm primeiro, na ordem desejada; os irmãos ausentes da lista
/// mantêm a ordem natural. A ordenação é estável e puramente determinística.
pub fn order_siblings(children: &mut [CatalogNode], desired: &[String]) {
    if desired.is_empty() {
        return;
    }
    let rank: BTreeMap<&str, usize> = desired
        .iter()
        .enumerate()
        .map(|(index, path)| (path.as_str(), index))
        .collect();
    children.sort_by(|left, right| {
        match (rank.get(left.path.as_str()), rank.get(right.path.as_str())) {
            (Some(left_rank), Some(right_rank)) => left_rank.cmp(right_rank),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => natural_cmp(&left.path, &right.path),
        }
    });
}

/// Aplica a preferência a uma árvore de nós, recursivamente.
///
/// A lista `nodes` de topo é tratada como filhos do pai de caminho vazio. Cada nó com
/// preferência registrada tem os filhos reordenados; a subárvore é então percorrida.
pub fn apply_order(nodes: &mut [CatalogNode], order: &CatalogOrder) {
    if let Some(desired) = order.children("") {
        order_siblings(nodes, desired);
    }
    for node in nodes.iter_mut() {
        apply_node(node, order);
    }
}

/// Aplica a preferência a um nó e a toda a sua subárvore.
fn apply_node(node: &mut CatalogNode, order: &CatalogOrder) {
    if let Some(desired) = order.children(&node.path) {
        order_siblings(&mut node.children, desired);
    }
    for child in node.children.iter_mut() {
        apply_node(child, order);
    }
}

/// Caminhos dos filhos do pai indicado, quando o pai existe na árvore.
///
/// A lista de topo é o pai de caminho vazio (`""`), a mesma convenção de
/// [`apply_order`]. Um pai que não existe na árvore devolve `None`, e o adaptador o
/// trata como alvo inválido em vez de criar uma preferência órfã.
pub fn sibling_paths(nodes: &[CatalogNode], parent: &str) -> Option<Vec<String>> {
    if parent.is_empty() {
        return Some(nodes.iter().map(|node| node.path.clone()).collect());
    }
    find_node(nodes, parent).map(|node| {
        node.children
            .iter()
            .map(|child| child.path.clone())
            .collect()
    })
}

/// Ancestral de caminho exato, em qualquer profundidade.
fn find_node<'a>(nodes: &'a [CatalogNode], path: &str) -> Option<&'a CatalogNode> {
    for node in nodes {
        if node.path == path {
            return Some(node);
        }
        if let Some(found) = find_node(&node.children, path) {
            return Some(found);
        }
    }
    None
}

/// Verdadeiro quando `desired` é uma permutação exata de `existing`.
///
/// Uma reordenação manual só pode trocar irmãos de posição: a lista precisa conter
/// exatamente os mesmos caminhos, sem repetição e sem omitir nem acrescentar ninguém.
/// Isso recusa movimentos entre pais diferentes e listas desatualizadas.
pub fn validate_siblings(existing: &[String], desired: &[String]) -> bool {
    if desired.len() != existing.len() {
        return false;
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for path in desired {
        *counts.entry(path.as_str()).or_insert(0) += 1;
    }
    if counts.len() != existing.len() {
        return false;
    }
    existing
        .iter()
        .all(|path| counts.get(path.as_str()) == Some(&1))
}
