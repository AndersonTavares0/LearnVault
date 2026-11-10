import { Download } from 'lucide-react';
import { buttonVariants } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { fileStatus, transport, type FileEntry } from '@/lib/transport';

/** Non-media files of a course or lesson, each with a download link when on disk. */
export function Materials({ files, courseId }: { files: FileEntry[]; courseId: string }) {
	if (!files.length) return <p className="text-muted-foreground">{tr('No support materials in this course.', 'Nenhum material de apoio neste curso.')}</p>;
	return (
		<ul className="divide-y rounded-lg border bg-card">
			{files.map((file) => {
				const url = file.availability?.trim().toLowerCase() === 'missing' ? null : transport.mediaUrl(file, courseId);
				const status = fileStatus(file);
				return (
					<li key={file.path} className="flex min-h-14 items-center gap-3 px-4 py-2">
						<span className="min-w-0 flex-1 break-words">{file.name}{status && <span className="ml-2 text-xs text-muted-foreground">({status})</span>}</span>
						{url && (
							<a href={url} download={file.name} target="_blank" rel="noopener noreferrer" className={buttonVariants({ variant: 'outline', size: 'sm' })} aria-label={tr(`Download ${file.name}`, `Baixar ${file.name}`)}>
								<Download />{tr('Download', 'Baixar')}
							</a>
						)}
					</li>
				);
			})}
		</ul>
	);
}

