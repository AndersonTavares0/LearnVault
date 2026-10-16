use std::{
    ffi::OsStr,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use thiserror::Error;

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
    parse_command_with_env(args, |key| std::env::var(key).ok())
}

fn parse_command_with_env<I, S, F>(args: I, env: F) -> Result<Command, ConfigError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    F: Fn(&str) -> Option<String>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let env_listen_addr = env("LISTEN_ADDR").unwrap_or_else(|| "127.0.0.1".to_owned());
    let env_port = env("PORT").unwrap_or_else(|| "8090".to_owned());
    let mut listen_addr = None;
    let data_dir = env("DATA_DIR").unwrap_or_else(|| "data".to_owned());
    let mut database_path = PathBuf::from(data_dir).join("learnvault.sqlite");
    let mut library_root = PathBuf::from(
        env("LIBRARY_DIR").unwrap_or_else(|| DEFAULT_LIBRARY_ROOT.to_owned()),
    );
    let mut web_root = None;
    let mut admin_username = env("ADMIN_USERNAME").filter(|value| !value.is_empty());
    let admin_password = env("ADMIN_PASSWORD").filter(|value| !value.is_empty());
    let env_cookie_secure = env("COOKIE_SECURE");
    let mut cookie_secure = None;
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
                listen_addr = Some(
                    value
                        .parse()
                        .map_err(|_| ConfigError::InvalidListenAddress)?,
                );
            }
            "--database" => database_path = PathBuf::from(next_value(&mut args)?),
            "--library-root" => library_root = PathBuf::from(next_value(&mut args)?),
            "--web-root" => web_root = Some(PathBuf::from(next_value(&mut args)?)),
            "--admin-username" => admin_username = Some(next_value(&mut args)?),
            "--admin-password" => {
                let _ = next_value(&mut args)?;
                return Err(ConfigError::InsecureAdminPasswordArgument);
            }
            "--insecure-http" => cookie_secure = Some(false),
            _ => return Err(ConfigError::UnknownArgument),
        }
    }

    if version_requested {
        return Ok(Command::Version);
    }

    if database_path.as_os_str().is_empty() {
        return Err(ConfigError::InvalidDatabasePath);
    }

    let listen_addr = listen_addr
        .unwrap_or(listen_addr_from_env(&env_listen_addr, &env_port)?);
    let cookie_secure = cookie_secure
        .or(env_cookie_secure
            .as_deref()
            .map(parse_bool)
            .transpose()?)
        .unwrap_or(true);

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

fn listen_addr_from_env(host: &str, port: &str) -> Result<SocketAddr, ConfigError> {
    if let Ok(address) = host.parse::<SocketAddr>() {
        return Ok(address);
    }

    let ip = host
        .parse::<IpAddr>()
        .map_err(|_| ConfigError::InvalidListenAddress)?;
    let port = port
        .parse::<u16>()
        .map_err(|_| ConfigError::InvalidListenAddress)?;
    Ok(SocketAddr::new(ip, port))
}

fn parse_bool(value: &str) -> Result<bool, ConfigError> {
    match value {
        value if value.eq_ignore_ascii_case("true") => Ok(true),
        value if value.eq_ignore_ascii_case("false") => Ok(false),
        _ => Err(ConfigError::InvalidArgument),
    }
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path};

    use super::{Command, ConfigError, parse_command_with_env};

    #[test]
    fn compose_environment_contract_builds_server_config() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("library");
        let data = temp.path().join("data");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        let environment = HashMap::from([
            ("LISTEN_ADDR", "0.0.0.0"),
            ("PORT", "8181"),
            ("DATA_DIR", data.to_str().unwrap()),
            ("LIBRARY_DIR", library.to_str().unwrap()),
            ("COOKIE_SECURE", "false"),
        ]);

        let command = parse_command_with_env(
            ["learnvault-server"],
            |key| environment.get(key).copied().map(str::to_owned),
        )
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert_eq!(config.listen_addr.to_string(), "0.0.0.0:8181");
        assert_eq!(config.database_path, data.join("learnvault.sqlite"));
        assert_eq!(config.library_root, canonical(&library));
        assert!(!config.cookie_secure);
    }

    #[test]
    fn command_line_values_override_compose_environment() {
        let temp = tempfile::tempdir().unwrap();
        let env_library = temp.path().join("env-library");
        let cli_library = temp.path().join("cli-library");
        let env_data = temp.path().join("env-data");
        std::fs::create_dir_all(&env_library).unwrap();
        std::fs::create_dir_all(&cli_library).unwrap();
        let environment = HashMap::from([
            ("LISTEN_ADDR", "not-an-address"),
            ("PORT", "not-a-port"),
            ("DATA_DIR", env_data.to_str().unwrap()),
            ("LIBRARY_DIR", env_library.to_str().unwrap()),
            ("COOKIE_SECURE", "not-a-bool"),
        ]);

        let command = parse_command_with_env(
            [
                "learnvault-server",
                "--listen-addr",
                "127.0.0.1:9090",
                "--database",
                "custom.sqlite",
                "--library-root",
                cli_library.to_str().unwrap(),
                "--insecure-http",
            ],
            |key| environment.get(key).copied().map(str::to_owned),
        )
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert_eq!(config.listen_addr.to_string(), "127.0.0.1:9090");
        assert_eq!(config.database_path, "custom.sqlite".into());
        assert_eq!(config.library_root, canonical(&cli_library));
        assert!(!config.cookie_secure);
    }

    #[test]
    fn invalid_cookie_secure_is_rejected_without_exposing_value() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let environment = HashMap::from([
            ("LIBRARY_DIR", library.to_str().unwrap()),
            ("COOKIE_SECURE", "not-a-secret-value"),
        ]);

        let error = parse_command_with_env(
            ["learnvault-server"],
            |key| environment.get(key).copied().map(str::to_owned),
        )
        .unwrap_err();

        assert_eq!(error, ConfigError::InvalidArgument);
        assert!(!error.to_string().contains("not-a-secret-value"));
    }

    #[test]
    fn empty_bootstrap_credentials_use_the_safe_first_startup_flow() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let environment = HashMap::from([
            ("LIBRARY_DIR", library.to_str().unwrap()),
            ("ADMIN_USERNAME", ""),
            ("ADMIN_PASSWORD", ""),
        ]);

        let command = parse_command_with_env(
            ["learnvault-server"],
            |key| environment.get(key).copied().map(str::to_owned),
        )
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert_eq!(config.admin_username, None);
        assert_eq!(config.admin_password, None);
    }

    fn canonical(path: &Path) -> std::path::PathBuf {
        path.canonicalize().unwrap()
    }
}
