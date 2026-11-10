import { Printer } from 'lucide-react';
import { useMemo } from 'react';
import { Link, useParams } from 'react-router';
import { BrandMark } from '@/components/brand';
import { Button, buttonVariants } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { countCatalogNode } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { findLessonLocation } from '@/lib/router';
import { useCatalog, useRecent, useSession } from '@/app/data';

/**
 * Local completion certificate, built from progress already on this server and printed by
 * the browser. It says plainly that it is a personal record, not an accredited credential.
 */
export function CertificatePage() {
	useLocale();
	const { courseId = '' } = useParams();
	const session = useSession();
	const catalog = useCatalog();
	const recent = useRecent();
	const entry = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set() }).entries.find((item) => item.courseId === courseId), [catalog.data, recent.data, courseId]);
	// The date of the last lesson completed in this course.
	const finishedAt = useMemo(() => Math.max(0, ...(recent.data ?? []).filter((item) => item.completed && item.updatedAt && findLessonLocation(catalog.data ?? null, item.mediaId)?.courseId === courseId).map((item) => item.updatedAt!)), [recent.data, catalog.data, courseId]);

	if (catalog.isPending || recent.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>;
	if (!entry || entry.status !== 'completed') {
		return (
			<div className="space-y-4">
				<h1 className="text-2xl font-semibold">{tr('No certificate yet', 'Ainda sem certificado')}</h1>
				<p className="text-muted-foreground">{tr('A certificate is available once every lesson of the course is completed.', 'O certificado fica disponível quando todas as aulas do curso forem concluídas.')}</p>
				<Link to="/progress" className={buttonVariants({ variant: 'outline' })}>{tr('Back to Progress', 'Voltar para Progresso')}</Link>
			</div>
		);
	}

	const counts = countCatalogNode(entry.course);
	const date = finishedAt ? new Date(finishedAt * 1000).toLocaleDateString(getLocale(), { dateStyle: 'long' }) : '';
	return (
		<div className="space-y-6">
			<div className="flex flex-wrap gap-2 print:hidden">
				<Link to="/progress" className={buttonVariants({ variant: 'outline' })}>{tr('Back to Progress', 'Voltar para Progresso')}</Link>
				<Button onClick={() => window.print()}><Printer />{tr('Print or save as PDF', 'Imprimir ou salvar como PDF')}</Button>
			</div>
			<article className="mx-auto max-w-3xl space-y-8 rounded-xl border-2 border-primary bg-card p-10 text-center print:border-black">
				<BrandMark className="mx-auto size-14 text-primary" />
				<p className="text-sm tracking-widest text-muted-foreground uppercase">{tr('Certificate of completion', 'Certificado de conclusão')}</p>
				<h1 className="text-3xl font-semibold text-balance">{entry.course.title}</h1>
				<p className="text-lg">
					{tr('Completed by', 'Concluído por')} <strong>{session.data?.user?.username}</strong>
					{date && <> · {date}</>}
				</p>
				<p className="text-muted-foreground">{tr(`${counts.lessons} lessons`, `${counts.lessons} aulas`)}</p>
				<p className="text-xs text-muted-foreground">{tr('Issued by this LearnVault library from your own study record. It is a personal record, not an accredited credential.', 'Emitido por esta biblioteca LearnVault a partir do seu próprio registro de estudo. É um registro pessoal, não uma credencial acreditada.')}</p>
			</article>
		</div>
	);
}
