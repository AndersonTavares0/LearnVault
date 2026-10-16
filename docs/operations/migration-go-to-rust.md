# Migração do SQLite Go para Rust

O núcleo Rust oferece uma importação explícita e limitada por biblioteca. Ela não é executada
automaticamente pelo servidor e não modifica o banco Go nem a biblioteca física.

`learnvault_core::import_go_database(LegacyImportOptions)` recebe `input`, `output`, `backup` e
`library_id`. O backup é criado primeiro; a saída fica em arquivo temporário e só é promovida
depois de validações de integridade, chaves estrangeiras e contagens. Os destinos devem ser novos
e distintos.

São importados usuários, sessões, arquivos de catálogo, remoções de mídia representáveis e
progresso. Caminhos são normalizados para `/`, IDs de usuários e mídia são gerados no banco Rust.
Como o schema Rust exige uma mídia para cada tombstone e não possui a coluna Go `is_media`, uma
remoção órfã, com `is_media` nulo ou com `is_media = 0` faz a operação falhar fechadamente; ela
não é descartada nem convertida em uma mídia fictícia. Colisões, papéis desconhecidos, timestamps
inválidos e valores fora dos limites também falham sem promover a saída.

O servidor CLI ainda não expõe um comando: a configuração atual não possui uma superfície
administrativa limpa para essa operação. A invocação deve ser feita por um adaptador que chame a
API do núcleo. Conserve o backup e confirme a saída com `Database::open` antes de usá-la.
