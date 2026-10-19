use std::{
    ffi::OsStr,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use thiserror::Error;

use learnvault_core::LegacyImportOptions;

/// Configuração validada necessária para iniciar o servidor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Endereço TCP do listener HTTP.
    pub listen_addr: SocketAddr,
    /// Caminho do banco SQLite.
    pub database_path: PathBuf,
    /// Raiz opcional usada para semear o primeiro curso quando o registro ainda não
    /// existe. É apenas uma conveniência de implantação: a gestão normal acontece pela
    /// interface, que registra cursos individualmente em um arquivo ao lado do banco.
    pub library_root: Option<PathBuf>,
    /// Raiz opcional dos arquivos estáticos da interface web.
    pub web_root: Option<PathBuf>,
    /// Usuário inicial usado somente quando o banco ainda não possui administrador.
    pub admin_username: Option<String>,
    /// Senha inicial usada somente quando o banco ainda não possui administrador.
    pub admin_password: Option<String>,
    /// Define se cookies de sessão recebem o atributo `Secure`.
    pub cookie_secure: bool,
    /// Define se o servidor consulta a release mais recente do LearnVault. Desligada,
    /// nenhuma chamada de rede sai do processo.
    pub update_check: bool,
}

/// Comando aceito pelo executável antes de iniciar o listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Imprime a versão compilada e encerra.
    Version,
    /// Importa um banco Go sem iniciar o servidor HTTP.
    MigrateGo(LegacyImportOptions),
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
    /// Uma credencial inicial não pôde ser carregada com segurança.
    #[error("invalid bootstrap credential configuration")]
    InvalidBootstrapCredential,
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
            Command::MigrateGo(_) => Err(ConfigError::InvalidArgument),
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
    let first_argument = args.next();
    if first_argument
        .as_ref()
        .and_then(|argument| argument.as_ref().to_str())
        == Some("migrate-go")
    {
        return parse_migrate_go(args);
    }
    let mut args = first_argument.into_iter().chain(args);
    let env_listen_addr = env("LISTEN_ADDR").unwrap_or_else(|| "127.0.0.1".to_owned());
    let env_port = env("PORT").unwrap_or_else(|| "8090".to_owned());
    let mut listen_addr = None;
    let data_dir = env("DATA_DIR").unwrap_or_else(|| "data".to_owned());
    let mut database_path = PathBuf::from(data_dir).join("learnvault.sqlite");
    let mut library_root = env("LIBRARY_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let mut web_root = None;
    let mut admin_username = bootstrap_credential(&env, "ADMIN_USERNAME")?;
    let admin_password = bootstrap_credential(&env, "ADMIN_PASSWORD")?;
    let env_cookie_secure = env("COOKIE_SECURE");
    let mut cookie_secure = None;
    let env_update_check = env("LEARNVAULT_UPDATE_CHECK");
    let mut update_check = None;
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
                        .parse::<SocketAddr>()
                        .map_err(|_| ConfigError::InvalidListenAddress)?,
                );
            }
            "--database" => database_path = PathBuf::from(next_value(&mut args)?),
            "--library-root" => library_root = Some(PathBuf::from(next_value(&mut args)?)),
            "--web-root" => web_root = Some(PathBuf::from(next_value(&mut args)?)),
            "--admin-username" => admin_username = Some(next_value(&mut args)?),
            "--admin-password" => {
                let _ = next_value(&mut args)?;
                return Err(ConfigError::InsecureAdminPasswordArgument);
            }
            "--insecure-http" => cookie_secure = Some(false),
            "--no-update-check" => update_check = Some(false),
            _ => return Err(ConfigError::UnknownArgument),
        }
    }

    if version_requested {
        return Ok(Command::Version);
    }

    if database_path.as_os_str().is_empty() {
        return Err(ConfigError::InvalidDatabasePath);
    }

    let listen_addr = match listen_addr {
        Some(address) => address,
        None => listen_addr_from_env(&env_listen_addr, &env_port)?,
    };
    let cookie_secure = match cookie_secure {
        Some(value) => value,
        None => env_cookie_secure
            .as_deref()
            .map(parse_bool)
            .transpose()?
            .unwrap_or(true),
    };
    let update_check = match update_check {
        Some(value) => value,
        None => match env_update_check.as_deref().map(str::trim) {
            None | Some("") => true,
            Some(value)
                if value.eq_ignore_ascii_case("off") || value.eq_ignore_ascii_case("false") =>
            {
                false
            }
            Some(value)
                if value.eq_ignore_ascii_case("on") || value.eq_ignore_ascii_case("true") =>
            {
                true
            }
            Some(_) => return Err(ConfigError::InvalidArgument),
        },
    };

    let library_root = library_root
        .map(|path| {
            path.canonicalize()
                .ok()
                .filter(|path| path.is_dir())
                .ok_or(ConfigError::InvalidLibraryRoot)
        })
        .transpose()?;
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
        update_check,
    }))
}

fn bootstrap_credential<F>(env: &F, name: &str) -> Result<Option<String>, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let direct = env(name).filter(|value| !value.is_empty());
    let file_path = env(&format!("{name}_FILE")).filter(|value| !value.is_empty());
    match (direct, file_path) {
        (Some(_), Some(_)) => Err(ConfigError::InvalidBootstrapCredential),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(file_path)) => {
            let mut value = std::fs::read_to_string(file_path)
                .map_err(|_| ConfigError::InvalidBootstrapCredential)?;
            if let Some(without_lf) = value.strip_suffix('\n') {
                value = without_lf
                    .strip_suffix('\r')
                    .unwrap_or(without_lf)
                    .to_owned();
            }
            if value.is_empty() {
                return Err(ConfigError::InvalidBootstrapCredential);
            }
            Ok(Some(value))
        }
        (None, None) => Ok(None),
    }
}

fn parse_migrate_go<I, S>(args: I) -> Result<Command, ConfigError>
where
    I: Iterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut input = None;
    let mut output = None;
    let mut backup = None;
    let mut library_id = None;

    let mut args = args;
    while let Some(argument) = args.next() {
        let argument = argument
            .as_ref()
            .to_str()
            .ok_or(ConfigError::InvalidArgument)?;
        let slot = match argument {
            "--input" => &mut input,
            "--output" => &mut output,
            "--backup" => &mut backup,
            "--library-id" => &mut library_id,
            _ => return Err(ConfigError::UnknownArgument),
        };
        if slot.is_some() {
            return Err(ConfigError::InvalidArgument);
        }
        let value = next_value(&mut args)?;
        if value.is_empty() {
            return Err(ConfigError::InvalidArgument);
        }
        *slot = Some(value);
    }

    let (Some(input), Some(output), Some(backup), Some(library_id)) =
        (input, output, backup, library_id)
    else {
        return Err(ConfigError::InvalidArgument);
    };
    if library_id.trim().is_empty() {
        return Err(ConfigError::InvalidArgument);
    }

    Ok(Command::MigrateGo(LegacyImportOptions {
        input: PathBuf::from(input),
        output: PathBuf::from(output),
        backup: PathBuf::from(backup),
        library_id,
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
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };

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

        let command = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert_eq!(config.listen_addr.to_string(), "0.0.0.0:8181");
        assert_eq!(config.database_path, data.join("learnvault.sqlite"));
        assert_eq!(config.library_root, Some(canonical(&library)));
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
        assert_eq!(config.database_path, PathBuf::from("custom.sqlite"));
        assert_eq!(config.library_root, Some(canonical(&cli_library)));
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

        let error = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
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

        let command = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert_eq!(config.admin_username, None);
        assert_eq!(config.admin_password, None);
    }

    #[test]
    fn bootstrap_credentials_can_be_loaded_from_secret_files() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("library");
        let username_file = temp.path().join("username");
        let password_file = temp.path().join("password");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::write(&username_file, "first-admin\r\n").unwrap();
        std::fs::write(&password_file, "synthetic-test-secret\n").unwrap();
        let environment = HashMap::from([
            ("LIBRARY_DIR", library.to_str().unwrap()),
            ("ADMIN_USERNAME_FILE", username_file.to_str().unwrap()),
            ("ADMIN_PASSWORD_FILE", password_file.to_str().unwrap()),
        ]);

        let command = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert_eq!(config.admin_username.as_deref(), Some("first-admin"));
        assert_eq!(
            config.admin_password.as_deref(),
            Some("synthetic-test-secret")
        );
    }

    #[test]
    fn direct_and_file_bootstrap_credentials_cannot_be_combined() {
        let temp = tempfile::tempdir().unwrap();
        let password_file = temp.path().join("password");
        std::fs::write(&password_file, "synthetic-test-secret").unwrap();
        let environment = HashMap::from([
            ("ADMIN_PASSWORD", "direct-secret"),
            ("ADMIN_PASSWORD_FILE", password_file.to_str().unwrap()),
        ]);

        let error = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap_err();

        assert!(!error.to_string().contains("direct-secret"));
        assert!(!error.to_string().contains(password_file.to_str().unwrap()));
    }

    #[test]
    fn missing_bootstrap_secret_file_fails_without_exposing_its_path() {
        let temp = tempfile::tempdir().unwrap();
        let missing_file = temp.path().join("missing-secret");
        let environment = HashMap::from([("ADMIN_PASSWORD_FILE", missing_file.to_str().unwrap())]);

        let error = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap_err();

        assert!(!error.to_string().contains(missing_file.to_str().unwrap()));
    }

    #[test]
    fn empty_bootstrap_secret_file_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let password_file = temp.path().join("password");
        std::fs::write(&password_file, "\n").unwrap();
        let environment = HashMap::from([("ADMIN_PASSWORD_FILE", password_file.to_str().unwrap())]);

        assert!(
            parse_command_with_env(["learnvault-server"], |key| {
                environment.get(key).copied().map(str::to_owned)
            })
            .is_err()
        );
    }

    #[test]
    fn parses_migrate_go_without_server_environment() {
        let command = parse_command_with_env(
            [
                "learnvault-server",
                "migrate-go",
                "--input",
                "go.sqlite",
                "--output",
                "rust.sqlite",
                "--backup",
                "go.sqlite.backup",
                "--library-id",
                "library-42",
            ],
            |_| None,
        )
        .unwrap();

        assert_eq!(
            command,
            Command::MigrateGo(learnvault_core::LegacyImportOptions {
                input: "go.sqlite".into(),
                output: "rust.sqlite".into(),
                backup: "go.sqlite.backup".into(),
                library_id: "library-42".into(),
            })
        );
    }

    #[test]
    fn migrate_go_requires_all_explicit_options() {
        let error = parse_command_with_env(
            ["learnvault-server", "migrate-go", "--input", "go.sqlite"],
            |_| None,
        )
        .unwrap_err();

        assert_eq!(error, ConfigError::InvalidArgument);
    }

    #[test]
    fn migrate_go_rejects_duplicate_and_unknown_options() {
        let duplicate = parse_command_with_env(
            [
                "learnvault-server",
                "migrate-go",
                "--input",
                "one.sqlite",
                "--input",
                "two.sqlite",
                "--output",
                "rust.sqlite",
                "--backup",
                "backup.sqlite",
                "--library-id",
                "library-1",
            ],
            |_| None,
        )
        .unwrap_err();
        assert_eq!(duplicate, ConfigError::InvalidArgument);

        let unknown = parse_command_with_env(
            [
                "learnvault-server",
                "migrate-go",
                "--input",
                "go.sqlite",
                "--output",
                "rust.sqlite",
                "--backup",
                "backup.sqlite",
                "--library-id",
                "library-1",
                "--listen-addr",
            ],
            |_| None,
        )
        .unwrap_err();
        assert_eq!(unknown, ConfigError::UnknownArgument);
    }

    #[test]
    fn update_check_defaults_to_enabled() {
        let command = parse_command_with_env(["learnvault-server"], |_| None).unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert!(config.update_check);
    }

    #[test]
    fn the_no_update_check_flag_wins_over_the_environment() {
        let environment = HashMap::from([("LEARNVAULT_UPDATE_CHECK", "on")]);

        let command = parse_command_with_env(["learnvault-server", "--no-update-check"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert!(!config.update_check);
    }

    #[test]
    fn the_update_check_environment_can_disable_the_check() {
        let environment = HashMap::from([("LEARNVAULT_UPDATE_CHECK", "off")]);

        let command = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap();
        let Command::Run(config) = command else {
            panic!("expected run command");
        };

        assert!(!config.update_check);
    }

    #[test]
    fn invalid_update_check_environment_is_rejected_without_exposing_value() {
        let environment = HashMap::from([("LEARNVAULT_UPDATE_CHECK", "not-a-secret-value")]);

        let error = parse_command_with_env(["learnvault-server"], |key| {
            environment.get(key).copied().map(str::to_owned)
        })
        .unwrap_err();

        assert_eq!(error, ConfigError::InvalidArgument);
        assert!(!error.to_string().contains("not-a-secret-value"));
    }

    fn canonical(path: &Path) -> std::path::PathBuf {
        path.canonicalize().unwrap()
    }
}
