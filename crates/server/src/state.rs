use crate::Config;

/// Estado compartilhado mínimo da aplicação HTTP.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Configuração validada usada pelo servidor.
    pub config: Config,
}

impl AppState {
    /// Cria o estado inicial da aplicação.
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}
