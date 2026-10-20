//! Preferências de ordem manual do catálogo, sem alterar os arquivos da biblioteca.

use std::collections::{HashMap, HashSet};

use rusqlite::params;

use crate::{CatalogNode, Database, Error, Result, paths::relative_path_key};

const MAX_ORDER_ITEMS: usize = 10_000;
const MAX_ORDER_PATH_BYTES: usize = 4_096;

/// Substitui atomicamente a ordem dos filhos de uma lista para um dono e biblioteca.
///
/// A camada de integração valida que `child_paths` são exatamente os irmãos exibidos.
/// Caminhos antigos são aceitos para preservar preferências quando uma varredura não
/// encontra um item.
pub fn persist_catalog_order(
    db: &Database,
    owner_id: &str,
    library_id: &str,
    parent_path: &str,
    child_paths: &[&str],
) -> Result<()> {
    validate_scope(owner_id, library_id, parent_path)?;
    if child_paths.is_empty() || child_paths.len() > MAX_ORDER_ITEMS {
        return Err(Error::InvalidCatalogOrder);
    }
    let mut unique = HashSet::with_capacity(child_paths.len());
    for child_path in child_paths {
        validate_path(child_path)?;
        if !is_direct_child(parent_path, child_path) || !unique.insert(*child_path) {
            return Err(Error::InvalidCatalogOrder);
        }
    }

    let tx = db.connection().unchecked_transaction()?;
    let stale_paths = {
        let mut statement = tx.prepare(
            "SELECT child_path FROM catalog_order
             WHERE owner_id = ?1 AND library_id = ?2 AND parent_path = ?3
             ORDER BY position",
        )?;
        let rows = statement.query_map(params![owner_id, library_id, parent_path], |row| {
            row.get::<_, String>(0)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|path| !unique.contains(path.as_str()))
            .collect::<Vec<_>>()
    };
    if child_paths.len() + stale_paths.len() > MAX_ORDER_ITEMS {
        return Err(Error::InvalidCatalogOrder);
    }
    tx.execute(
        "DELETE FROM catalog_order
         WHERE owner_id = ?1 AND library_id = ?2 AND parent_path = ?3",
        params![owner_id, library_id, parent_path],
    )?;
    {
        let mut statement = tx.prepare(
            "INSERT INTO catalog_order
                 (owner_id, library_id, parent_path, child_path, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (position, child_path) in child_paths
            .iter()
            .copied()
            .chain(stale_paths.iter().map(String::as_str))
            .enumerate()
        {
            statement.execute(params![
                owner_id,
                library_id,
                parent_path,
                child_path,
                position as i64
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Lê a preferência em posição crescente; uma lista sem preferência retorna vazia.
pub fn catalog_order(
    db: &Database,
    owner_id: &str,
    library_id: &str,
    parent_path: &str,
) -> Result<Vec<String>> {
    validate_scope(owner_id, library_id, parent_path)?;
    let mut statement = db.connection().prepare(
        "SELECT child_path FROM catalog_order
         WHERE owner_id = ?1 AND library_id = ?2 AND parent_path = ?3
         ORDER BY position",
    )?;
    let rows = statement.query_map(params![owner_id, library_id, parent_path], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Remove somente a preferência do dono, biblioteca e lista selecionados.
pub fn reset_catalog_order(
    db: &Database,
    owner_id: &str,
    library_id: &str,
    parent_path: &str,
) -> Result<()> {
    validate_scope(owner_id, library_id, parent_path)?;
    db.connection().execute(
        "DELETE FROM catalog_order
         WHERE owner_id = ?1 AND library_id = ?2 AND parent_path = ?3",
        params![owner_id, library_id, parent_path],
    )?;
    Ok(())
}

/// Reordena itens pela preferência e mantém itens novos no final, em ordem natural.
///
/// A fatia recebida já deve estar na ordem natural. Caminhos ausentes da preferência
/// preservam essa ordem natural entre si.
pub fn apply_catalog_order<T>(items: &mut [T], saved_order: &[String], path: impl Fn(&T) -> &str) {
    let positions: HashMap<&str, usize> = saved_order
        .iter()
        .enumerate()
        .map(|(position, child_path)| (child_path.as_str(), position))
        .collect();
    items.sort_by_key(|item| positions.get(path(item)).copied().unwrap_or(usize::MAX));
}

/// Aplica recursivamente a ordem persistida a uma árvore já montada e naturalmente ordenada.
pub fn apply_saved_catalog_order(
    db: &Database,
    owner_id: &str,
    library_id: &str,
    nodes: &mut [CatalogNode],
    parent_path: &str,
) -> Result<()> {
    let saved_order = catalog_order(db, owner_id, library_id, parent_path)?;
    apply_catalog_order(nodes, &saved_order, |node| &node.path);
    for node in nodes {
        apply_saved_catalog_order(db, owner_id, library_id, &mut node.children, &node.path)?;
    }
    Ok(())
}

fn validate_scope(owner_id: &str, library_id: &str, parent_path: &str) -> Result<()> {
    if owner_id.is_empty()
        || owner_id.len() > 256
        || library_id.is_empty()
        || library_id.len() > 256
        || (!parent_path.is_empty() && validate_path(parent_path).is_err())
    {
        return Err(Error::InvalidCatalogOrder);
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<()> {
    if path.len() > MAX_ORDER_PATH_BYTES
        || relative_path_key(path, crate::KeyMode::for_host()).is_err()
    {
        return Err(Error::InvalidCatalogOrder);
    }
    Ok(())
}

fn is_direct_child(parent_path: &str, child_path: &str) -> bool {
    let Some(relative) = child_path.strip_prefix(parent_path) else {
        return false;
    };
    let relative = if parent_path.is_empty() {
        relative
    } else {
        let Some(relative) = relative.strip_prefix('/') else {
            return false;
        };
        relative
    };
    !relative.is_empty() && !relative.contains('/')
}
