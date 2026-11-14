import { tr } from '@/lib/i18n';

/** Codes the server uses when a remote source cannot be read (connection test, folder browsing, scan). */
const FAILURE_CODES = ['unreachable', 'denied', 'not_found', 'invalid_response', 'limit_exceeded', 'credentials_unavailable', 'timeout'];

export const isSourceFailure = (code: string | null): code is string => code !== null && FAILURE_CODES.includes(code);

/** One localized sentence for a remote source failure code. */
export function sourceFailure(error: string | null): string {
	switch (error) {
		case 'unreachable': return tr('The server could not be reached. Check the address and the network.', 'Não foi possível alcançar o servidor. Confira o endereço e a rede.');
		case 'denied': return tr('The server refused these credentials.', 'O servidor recusou estas credenciais.');
		case 'not_found': return tr('The path was not found on the server.', 'O caminho não foi encontrado no servidor.');
		case 'invalid_response': return tr('The server did not answer as a WebDAV or SMB share.', 'O servidor não respondeu como um compartilhamento WebDAV ou SMB.');
		case 'limit_exceeded': return tr('The folder is too large to list.', 'A pasta é grande demais para listar.');
		case 'credentials_unavailable': return tr('The stored password cannot be read. Restore the encryption key or add the source again.', 'Não é possível ler a senha guardada. Restaure a chave de criptografia ou adicione a fonte de novo.');
		case 'timeout': return tr('The server took too long to answer.', 'O servidor demorou demais para responder.');
		default: return tr('The connection test failed.', 'O teste de conexão falhou.');
	}
}
