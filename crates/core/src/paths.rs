//! Normalização e resolução de caminhos relativos da biblioteca.
//!
//! O domínio nunca trata uma string de caminho como identidade: a identidade é o
//! `media_id`. O caminho relativo é apenas a localização atual do arquivo dentro da
//! biblioteca, e a chave de comparação (`relative_path_key`) é derivada dele.
//!
//! A resolução é **puramente lexical**: estas funções não tocam o sistema de arquivos.
//! O confinamento lexical é necessário, mas não é suficiente: a verificação do destino
//! real do arquivo — links simbólicos, junctions e fluxos alternativos do Windows —
//! pertence à entrega de resolução de mídia, que confere o objeto aberto.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Modo de comparação de caminhos.
///
/// O modo é um parâmetro explícito, e não uma consulta ao sistema operacional, para que
/// os dois comportamentos possam ser verificados no mesmo ambiente de testes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// Diferencia maiúsculas de minúsculas.
    Sensitive,
    /// Ignora maiúsculas de minúsculas.
    Insensitive,
}

impl KeyMode {
    /// Modo de comparação do sistema operacional atual.
    ///
    /// O Windows não diferencia maiúsculas de minúsculas no caminho e, entre os alvos
    /// declarados do projeto, é o único sistema onde isso é a regra. Os demais alvos
    /// preservam a caixa no sistema de arquivos padrão; uma montagem que não preserve
    /// (por exemplo um compartilhamento ou um volume montado) continua sendo tratada
    /// como sensível, e isso está registrado como limitação conhecida.
    pub fn for_host() -> Self {
        if cfg!(windows) {
            KeyMode::Insensitive
        } else {
            KeyMode::Sensitive
        }
    }
}

/// Normaliza um caminho relativo e produz a chave de comparação.
///
/// O caminho precisa ser relativo e não pode conter componentes `.` ou `..`, componentes
/// vazios, bytes nulos ou componentes com prefixo de unidade (`C:`). Separadores `\` e
/// `/` são convertidos para `/`. No modo [`KeyMode::Insensitive`] apenas letras ASCII
/// são dobradas para minúsculas, o mesmo comportamento de `COLLATE NOCASE` no SQLite.
///
/// Separações são normalizadas para `/` em qualquer sistema, para que a chave de um
/// mesmo arquivo seja a mesma quando a biblioteca é usada em plataformas diferentes.
/// Isso significa que um nome de arquivo que contenha `\` no Unix produz a mesma chave
/// de um caminho aninhado equivalente; a colisão é detectada pela restrição de
/// unicidade do banco, e o caso está registrado como limitação conhecida.
pub fn relative_path_key(path: &str, mode: KeyMode) -> Result<String> {
    let normalized = normalize_relative(path)?;

    Ok(match mode {
        KeyMode::Sensitive => normalized,
        KeyMode::Insensitive => normalized.to_ascii_lowercase(),
    })
}

/// Indica se uma chave de caminho está na área indicada — a própria área ou algo dentro
/// dela.
///
/// A comparação respeita a fronteira de componente: `A` cobre `A` e `A/x`, mas não `AB`.
/// É usada pela política de rescan e pelo catálogo ao tratar áreas não verificadas.
pub(crate) fn is_under_area(key: &str, area: &str) -> bool {
    key == area
        || (key.len() > area.len() && key.starts_with(area) && key.as_bytes()[area.len()] == b'/')
}

/// Resolve um caminho relativo dentro da raiz da biblioteca.
///
/// A função confina o resultado por análise lexical e recusa caminhos absolutos, com
/// componentes `.` ou `..`, vazios, malformados ou com prefixo de unidade. Nenhum acesso
/// ao sistema de arquivos é feito aqui.
pub fn resolve_within(root: &Path, relative: &str) -> Result<PathBuf> {
    let normalized = normalize_relative(relative)?;

    let mut resolved = root.to_path_buf();
    for component in normalized.split('/') {
        resolved.push(component);
    }

    // Defesa em profundidade: no Windows, `PathBuf::push` descarta o caminho acumulado
    // quando o componente tem prefixo próprio (por exemplo `C:`). A validação por
    // componente já recusa esse caso; esta checagem mantém o contrato verdadeiro mesmo
    // se a validação mudar.
    if !resolved.starts_with(root) {
        return Err(Error::InvalidRelativePath {
            path: relative.to_owned(),
        });
    }

    Ok(resolved)
}

fn normalize_relative(path: &str) -> Result<String> {
    if path.is_empty() || is_absolute_like(path) {
        return Err(Error::InvalidRelativePath {
            path: path.to_owned(),
        });
    }

    let mut components = Vec::new();
    for component in path.split(['/', '\\']) {
        // A validação é por componente, não apenas no início da string: um componente
        // como `C:` no meio do caminho também descarta a raiz ao ser empilhado.
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains('\0')
            || is_absolute_like(component)
        {
            return Err(Error::InvalidRelativePath {
                path: path.to_owned(),
            });
        }
        components.push(component);
    }

    Ok(components.join("/"))
}

fn is_absolute_like(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || has_drive_prefix(path)
}

fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
