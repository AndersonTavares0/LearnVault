import { useQuery } from '@tanstack/react-query';
import { getVersion } from '@tauri-apps/api/app';
import { Motto } from '@/components/brand';
import { tr } from '@/lib/i18n';

const loadVersion = __LEARNVAULT_TRANSPORT__ === 'desktop'
	? getVersion
	: async (): Promise<string> => {
		const response = await fetch('/api/healthz');
		if (!response.ok) throw new Error(tr('Version unavailable.', 'Versão indisponível.'));
		const { version } = await response.json();
		return version;
	};

export function AboutSettings() {
	const version = useQuery({ queryKey: ['app-version'], queryFn: loadVersion, staleTime: Infinity, retry: false });

	return (
		<div className="space-y-2">
			<p className="font-heading text-xl font-semibold tracking-tight">LearnVault</p>
			<p className="text-sm text-muted-foreground" role="status">
				{version.data
					? tr(`Version ${version.data}`, `Versão ${version.data}`)
					: version.isPending
						? tr('Loading version…', 'Carregando versão…')
						: tr('Version unavailable.', 'Versão indisponível.')}
			</p>
			<Motto name="sapereAude" />
			<p className="text-sm">{tr('LearnVault only reads your course folders.', 'O LearnVault apenas lê suas pastas de cursos.')}</p>
			<a href="https://github.com/AndersonTavares0/LearnVault" target="_blank" rel="noopener noreferrer" className="inline-flex min-h-11 items-center text-sm font-medium underline underline-offset-4">
				{tr('Project repository', 'Repositório do projeto')}
			</a>
		</div>
	);
}
