use std::{ffi::OsStr, net::SocketAddr, path::PathBuf};

use thiserror::Error;

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8090";
const DEFAULT_DATABASE_PATH: &str = "data/learnvault.sqlite";
const DEFAULT_LIBRARY_ROOT: &str = "library";

/// Configuração validada necessária para iniciar o servidor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Endereço TCP do listener HTTP.
    pub listen_addr: SocketAddr,
    /// Caminho do banco SQLite.
    pub database_path: PathBuf,
    /// Raiz autorizada da biblioteca física.
    pub library_root: PathBuf,
    /// Raiz opcional dos arquivos estáticos da interface web.
    pub web_root: Option<PathBuf>,
    /// Usuário inicial usado somente quando o banco ainda não possui administrador.
    pub admin_username: Option<String>,
    /// Senha inicial usada somente quando o banco ainda não possui administrador.
    pub admin_password: Option<String>,
    /// Define se cookies de sessão recebem o atributo `Secure`.
    pub cookie_secure: bool,
}

/// Comando aceito pelo executável antes de iniciar o listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Imprime a versão compilada e encerra.
    Version,
    /// Inicia o servidor com a configuração validada.
    Run(Config),
}

/// Erros seguros de configuração e argumentos.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// O endereço de escuta não pôde ser interpretado.
    #[error("invalid listen address")]
    InvalidListenAddress,
    /// O caminho do banco está vazio ou contém valor inválido.
    #[error("invalid database path")]
    InvalidDatabasePath,
    /// A raiz da biblioteca não existe ou não é um diretório.
    #[error("invalid library root")]
    InvalidLibraryRoot,
    /// A raiz da interface web não existe ou não é um diretório.
    #[error("invalid web root")]
    InvalidWebRoot,
    /// Um argumento exige um valor seguinte.
    #[error("missing argument value")]
    MissingArgumentValue,
    /// O argumento não é reconhecido.
    #[error("unknown argument")]
    UnknownArgument,
    /// O argumento contém texto que não pôde ser interpretado.
    #[error("invalid argument")]
    InvalidArgument,
    /// Segredo não pode ser exposto na lista de processos.
    #[error("admin password must be supplied through ADMIN_PASSWORD or a secret file")]
    InsecureAdminPasswordArgument,
    /// `--version` não representa uma configuração de execução.
    #[error("version requested")]
    VersionRequested,
}

impl Config {
    /// Lê e valida uma configuração a partir de argumentos de linha de comando.
    pub fn from_args<I, S>(args: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        match parse_command(args)? {
            Command::Run(config) => Ok(config),
            Command::Version => Err(ConfigError::VersionRequested),
        }
    }
}

/// Interpreta os argumentos do executável sem iniciar o servidor.
pub fn parse_command<I, S>(args: I) -> Result<Command, ConfigError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let mut listen_addr = DEFAULT_LISTEN_ADDR.parse().expect("endereço padrão válido");
    let mut database_path = PathBuf::from(DEFAULT_DATABASE_PATH);
    let mut library_root = PathBuf::from(DEFAULT_LIBRARY_ROOT);
    let mut web_root = None;
    let mut admin_username = std::env::var("ADMIN_USERNAME").ok();
    let mut admin_password = std::env::var("ADMIN_PASSWORD").ok();
    let mut cookie_secure = true;
    let mut version_requested = false;

    while let Some(argument) = args.next() {
        let argument = argument
            .as_ref()
            .to_str()
            .ok_or(ConfigError::InvalidArgument)?;
        match argument {
            "--version" => version_requested = true,
            "--listen-addr" => {
                let value = next_value(&mut args)?;
                listen_addr = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidListenAddress)?;
            }
            "--database" => database_path = PathBuf::from(next_value(&mut args)?),
            "--library-root" => library_root = PathBuf::from(next_value(&mut args)?),
            "--web-root" => web_root = Some(PathBuf::from(next_value(&mut args)?)),
            "--admin-username" => admin_username = Some(next_value(&mut args)?),
            "--admin-password" => {
                let _ = next_value(&mut args)?;
                return Err(ConfigError::InsecureAdminPasswordArgument);
            }
            "--insecure-http" => cookie_secure = false,
            _ => return Err(ConfigError::UnknownArgument),
        }
    }

    if version_requested {
        return Ok(Command::Version);
    }

    if database_path.as_os_str().is_empty() {
        return Err(ConfigError::InvalidDatabasePath);
    }

    let library_root = library_root
        .canonicalize()
        .ok()
        .filter(|path| path.is_dir())
        .ok_or(ConfigError::InvalidLibraryRoot)?;
    let web_root = web_root
        .map(|path| {
            path.canonicalize()
                .ok()
                .filter(|path| path.is_dir())
                .ok_or(ConfigError::InvalidWebRoot)
        })
        .transpose()?;

    Ok(Command::Run(Config {
        listen_addr,
        database_path,
        library_root,
        web_root,
        admin_username,
        admin_password,
        cookie_secure,
    }))
}

fn next_value<I, S>(args: &mut I) -> Result<String, ConfigError>
where
    I: Iterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.next()
        .ok_or(ConfigError::MissingArgumentValue)?
        .as_ref()
        .to_str()
        .map(str::to_owned)
        .ok_or(ConfigError::InvalidArgument)
}
