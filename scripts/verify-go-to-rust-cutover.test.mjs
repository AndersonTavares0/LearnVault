// Contrato de segurança do ensaio de cutover Go->Rust (scripts/verify-go-to-rust-cutover.sh).
//
// O ensaio é manual e não publicante: ele constrói as duas imagens, gera um banco Go v8
// sintético exercitando o runtime Go, importa com `migrate-go` e sobe o OCI Rust experimental
// contra uma fixture temporária. Este teste NÃO executa Docker/Podman nem publica nada: ele
// trava os invariantes de segurança por leitura estática do script e do runbook, para que uma
// futura edição não reintroduza dados reais, `push`/`login`, montagens cruzadas ou promoção de
// saída sem verificação.
//
// Execução: `pnpm test:scripts` (ou `node --test scripts/verify-go-to-rust-cutover.test.mjs`).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const scriptPath = path.join(root, 'scripts', 'verify-go-to-rust-cutover.sh');
const docPath = path.join(root, 'docs', 'operations', 'migration-go-to-rust.md');

const script = readFileSync(scriptPath, 'utf8');
const doc = readFileSync(docPath, 'utf8');

// A substring de scripts já é suficiente para os invariantes; nada é executado aqui.

test('o ensaio é POSIX sh e usa apenas fixtures temporárias sintéticas', () => {
	assert.match(script, /^#!\/bin\/sh/, 'deve ser POSIX sh para rodar no host e no MSYS');
	assert.match(script, /mktemp -d/, 'a fixture deve nascer em um diretório temporário');
	assert.match(script, /guard_fixture_path/, 'os caminhos precisam passar pelo guard de fixture');
	assert.doesNotMatch(script, /\/mnt\/cursos/, 'nunca pode referenciar a biblioteca real');
	assert.doesNotMatch(script, /\bsqlite3\b/, 'não pode construir o banco com sqlite3');
	assert.doesNotMatch(script, /CREATE\s+TABLE/i, 'não pode embutir esquema/banco manualmente');
});

test('recusa caminhos de dados reais fora da fixture temporária', () => {
	assert.match(script, /refusing real-data path/, 'deve recusar explicitamente dados reais');
	assert.match(
		script,
		/\/mnt\/\*/,
		'o guard precisa rejeitar a árvore real /mnt mesmo sem citar /mnt/cursos'
	);
});

test('não publica nem autentica em registry', () => {
	assert.doesNotMatch(script, /\bpush\b/, 'nenhum caminho pode publicar a imagem');
	assert.doesNotMatch(script, /(?:\$engine|docker|podman)\s+login\b/, 'não pode fazer login em registry');
	assert.doesNotMatch(script, /git\s+push/, 'não pode empurrar commits');
});

test('constrói o Containerfile Go de referência e o Containerfile.rust experimental', () => {
	assert.match(script, /deploy\/container\/Containerfile\.rust/);
	assert.match(script, /deploy\/container\/Containerfile(?!\.rust)/, 'o runtime Go é o de referência');
});

test('os diretórios de dados Go e Rust são distintos e a colisão falha fechado', () => {
	assert.match(script, /go_data_mount/);
	assert.match(script, /rust_data_mount/);
	assert.match(
		script,
		/Go and Rust data directories resolve identically/,
		'deve falhar se os dois diretórios de dados resolverem para o mesmo caminho'
	);
	// Cada runtime usa o seu próprio volume de dados em /data.
	assert.match(script, /go_data_mount=.*:\/data:rw/);
	assert.match(script, /rust_data_mount=.*:\/data:rw/);
	assert.match(script, /-v\s+"\$go_data_mount"/);
	assert.match(script, /-v\s+"\$rust_data_mount"/);
});

test('gera o banco Go v8 exercitando o runtime Go e o quiesce antes de importar', () => {
	assert.match(script, /\/api\/admin\/catalog\/rescan/, 'o ensaio precisa catalogar a fixture via Go');
	assert.match(script, /\/api\/healthz/, 'o runtime Go precisa subir e responder');
	assert.match(
		script,
		/stop\s+"\$GO_CONTAINER"/,
		'o escritor Go precisa ser parado antes de usar o banco como entrada'
	);
	assert.match(script, /quiesc/i, 'a quiescência deve estar explícita');
	// A ordem importa: o Go para antes do `migrate-go`.
	assert.ok(
		script.indexOf('/api/admin/catalog/rescan') < script.indexOf('--output /output/learnvault.sqlite'),
		'o rescan Go precisa acontecer antes da importação'
	);
	assert.ok(
		script.indexOf('stop "$GO_CONTAINER"') < script.indexOf('--output /output/learnvault.sqlite'),
		'o Go precisa ser parado antes do `migrate-go`'
	);
});

test('invoca migrate-go pela imagem Rust com os quatro destinos', () => {
	assert.match(script, /--entrypoint \/app\/learnvault-server/, 'precisa sobrepor o entrypoint do servidor');
	assert.match(script, /migrate-go/);
	assert.match(script, /--input \/input\/go\.sqlite/);
	assert.match(script, /--output \/output\/learnvault\.sqlite/);
	assert.match(script, /--backup \/backup\/go\.sqlite\.backup/);
	assert.match(script, /--library-id "\$LIBRARY_ID"/);
	assert.match(script, /LIBRARY_ID="library-1"/);
});

test('confere relatório, checksum da origem e destinos distintos', () => {
	assert.match(script, /checksum/, 'precisa conferir o checksum da entrada');
	assert.match(script, /users=/, 'precisa verificar o relatório da importação');
	assert.match(script, /distinct/, 'precisa afirmar que saída e backup são distintos');
	assert.match(script, /sha256sum|shasum|cksum/, 'precisa de uma ferramenta de checksum');
});

test('sobe o OCI Rust endurecido contra a saída preparada em /data', () => {
	assert.match(script, /--read-only/);
	assert.match(script, /--security-opt no-new-privileges/);
	assert.match(script, /--cap-drop ALL/);
	assert.match(script, /:\/library:ro/, 'a biblioteca precisa ser somente leitura');
	assert.match(script, /cp\s+"\$rust_output"\s+"\$rust_data\/learnvault\.sqlite"/, 'a saída precisa ser preparada');
	assert.match(script, /--database \/data\/learnvault\.sqlite/);
	assert.match(script, /Range: bytes=/, 'precisa validar requisição de faixa de mídia');
	assert.match(script, /\/api\/study\/position/);
	assert.match(script, /\/api\/study\/progress/);
	assert.match(script, /restart/, 'precisa validar persistência após reinício');
	assert.match(script, /10001/, 'precisa validar o usuário não-root');
});

test('tem um caminho negativo fail-closed que não promove saída', () => {
	assert.match(script, /Negative path/, 'o caminho negativo deve estar explícito');
	assert.match(script, /no output was promoted/, 'o negativo precisa provar que nada foi promovido');
	assert.match(script, /negative\.sqlite/);
	assert.match(script, /negative\.backup/);
});

test('nunca registra a senha sintética', () => {
	assert.doesNotMatch(script, /set -x/, 'não pode habilitar traço de shell com credenciais');
	assert.doesNotMatch(
		script,
		/(?:echo|printf)[^\n]*\$\{?ADMIN_PASSWORD/,
		'a senha sintética não pode ser impressa'
	);
	assert.match(script, /-e "ADMIN_PASSWORD=\$ADMIN_PASSWORD"/, 'a senha só entra pelo ambiente do contêiner');
	assert.match(script, /learnvault_session/, 'a sessão segue o cookie do produto');
});

test('normaliza TMPDIR/TEMP/TMP inseguros antes de criar qualquer arquivo', () => {
	assert.match(script, /is_unsafe_temp_root/, 'precisa de um predicado de raiz temporária insegura');
	assert.match(script, /sanitize_temp_environment/, 'precisa normalizar o ambiente temporário');
	assert.match(script, /TMPDIR/, 'precisa inspecionar TMPDIR');
	assert.match(script, /TEMP\b/, 'precisa inspecionar TEMP');
	assert.match(script, /\bTMP\b/, 'precisa inspecionar TMP');

	// O preflight precisa recusar /mnt e árvores cursos e descartar o valor inseguro.
	const preflight = script.slice(
		script.indexOf('is_unsafe_temp_root'),
		script.indexOf('workdir=$(mktemp -d)')
	);
	assert.match(preflight, /\/mnt/, 'o preflight precisa recusar /mnt antes do mktemp');
	assert.match(preflight, /cursos/, 'o preflight precisa recusar árvores cursos antes do mktemp');
	assert.match(preflight, /unset/, 'o preflight precisa descartar o valor inseguro');

	// A limpeza precisa rodar ANTES do primeiro mktemp (nenhuma mutação antes dela).
	assert.ok(
		script.lastIndexOf('sanitize_temp_environment') < script.indexOf('workdir=$(mktemp -d)'),
		'a normalização precisa rodar antes do mktemp'
	);
	// Defesa em profundidade: o próprio workdir criado também passa pelo guard de fixture.
	assert.ok(
		script.indexOf('guard_fixture_path "$workdir"') !== -1 &&
			script.indexOf('guard_fixture_path "$workdir"') < script.indexOf('mkdir -p "$library'),
		'o workdir criado precisa passar pelo guard antes de qualquer mkdir'
	);
});

test('recusa credenciais do chamador e usa valores sintéticos fixos', () => {
	assert.doesNotMatch(
		script,
		/ADMIN_USERNAME="\$\{ADMIN_USERNAME:-/,
		'não pode herdar ADMIN_USERNAME do ambiente'
	);
	assert.doesNotMatch(
		script,
		/ADMIN_PASSWORD="\$\{ADMIN_PASSWORD:-/,
		'não pode herdar ADMIN_PASSWORD do ambiente'
	);
	assert.match(script, /ADMIN_USERNAME:-\}/, 'precisa detectar ADMIN_USERNAME do ambiente para recusar');
	assert.match(script, /ADMIN_PASSWORD:-\}/, 'precisa detectar ADMIN_PASSWORD do ambiente para recusar');
	assert.match(script, /refusing caller-supplied/i, 'precisa recusar credenciais do chamador');
	assert.match(script, /ADMIN_USERNAME="admin"/, 'deve fixar o usuário sintético');
	assert.match(script, /ADMIN_PASSWORD="learnvault-rehearsal-pass"/, 'deve fixar a senha sintética');
	assert.ok(
		script.indexOf('refusing caller-supplied') <
			script.indexOf('"$engine" run -d --name "$GO_CONTAINER"'),
		'o guard de credenciais precisa rodar antes de iniciar qualquer contêiner'
	);
});

test('o runbook documenta o ensaio com escopo, quiescência, rollback e limites', () => {
	assert.match(doc, /scripts\/verify-go-to-rust-cutover\.sh/, 'o comando exato precisa estar no runbook');
	assert.match(doc, /quiesc/i, 'a quiescência do escritor Go precisa estar documentada');
	assert.match(doc, /distint/i, 'os volumes/caminhos distintos precisam estar documentados');
	assert.match(doc, /rollback/i, 'o rollback precisa estar documentado');
	assert.match(doc, /RPO/, 'o RPO após escritas no Rust precisa estar documentado');
	assert.match(doc, /ensaio/i, 'o runbook precisa caracterizar como ensaio');
	assert.match(doc, /TMPDIR/, 'a normalização do diretório temporário precisa estar documentada');
	assert.match(
		doc,
		/ADMIN_USERNAME|ADMIN_PASSWORD/,
		'a recusa de credenciais do chamador precisa estar documentada'
	);
	assert.match(
		doc,
		/não comprov|permanece.*não|continua.*não|não prova|não comprova/i,
		'os limites do que não foi comprovado precisam estar explícitos'
	);
	assert.doesNotMatch(
		doc,
		/pront[oa] para produ|production[- ]ready|release[- ]ready|pronto para release/i,
		'não pode alegar prontidão de produção ou de release'
	);
});
