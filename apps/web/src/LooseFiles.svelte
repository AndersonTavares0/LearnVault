<script lang="ts">
 import { fileStatus, transport, type FileEntry } from './transport';
 import { href } from './router';
 let { files, heading, courseId }: { files: FileEntry[]; heading: string; courseId?: string } = $props();

 // O rótulo vem da extensão: "poster.jpg" sozinho não diz se a peça é imagem,
 // documento ou vídeo, e a lista de materiais é a única pista de que existe.
 const TIPOS: Record<string, string> = {
  mp4: 'Vídeo', webm: 'Vídeo', mkv: 'Vídeo', mov: 'Vídeo', avi: 'Vídeo', m4v: 'Vídeo',
  mp3: 'Áudio', wav: 'Áudio', m4a: 'Áudio', flac: 'Áudio', ogg: 'Áudio', oga: 'Áudio', aac: 'Áudio',
  jpg: 'Imagem', jpeg: 'Imagem', png: 'Imagem', gif: 'Imagem', webp: 'Imagem', svg: 'Imagem', bmp: 'Imagem',
  srt: 'Legenda', vtt: 'Legenda', ass: 'Legenda', ssa: 'Legenda', sub: 'Legenda',
  pdf: 'PDF', html: 'HTML', htm: 'HTML', txt: 'Texto', md: 'Texto', rtf: 'Texto',
  csv: 'CSV', json: 'JSON', xml: 'XML', yaml: 'YAML', yml: 'YAML',
  zip: 'Comprimido', rar: 'Comprimido', '7z': 'Comprimido', tar: 'Comprimido', gz: 'Comprimido',
  doc: 'Documento', docx: 'Documento', xls: 'Planilha', xlsx: 'Planilha', ppt: 'Apresentação', pptx: 'Apresentação'
 };

 function tipoDe(file: FileEntry): string {
  const extensao = file.name.split('.').pop()?.toLowerCase() ?? '';
  if (extensao.length === 0) return 'Arquivo';
  return TIPOS[extensao] ?? 'Arquivo';
 }
</script>
<section class="loose-files" aria-label={heading}>
	<strong>{heading}</strong>
	<ul>
		{#each files as file}
			{@const missing = file.availability?.trim().toLowerCase() === 'missing'}
			{@const url = missing ? null : transport.mediaUrl(file, courseId)}
			{@const status = fileStatus(file)}
			<li>
				{#if url && file.kind === 'media'}
					{#if courseId}
						<a class="file-row" href={href('course', courseId, file.path)}>
							<span class="file-type">{tipoDe(file)}</span>
							<span class="file-name">{file.name}</span>
							{#if status}<span class="file-status">{status}</span>{/if}
							<span class="file-action">Reproduzir</span>
						</a>
					{:else}
						<a class="file-row" href={href('player', file.path)}>
							<span class="file-type">{tipoDe(file)}</span>
							<span class="file-name">{file.name}</span>
							{#if status}<span class="file-status">{status}</span>{/if}
							<span class="file-action">Reproduzir</span>
						</a>
					{/if}
				{:else if url}
					<a class="file-row" href={url} download={file.name} target="_blank" rel="noopener noreferrer">
						<span class="file-type">{tipoDe(file)}</span>
						<span class="file-name">{file.name}</span>
						{#if status}<span class="file-status">{status}</span>{/if}
						<span class="file-action">Baixar</span>
					</a>
				{:else}
					<span class="file-row is-missing">
						<span class="file-type">{tipoDe(file)}</span>
						<span class="file-name">{file.name}</span>
						{#if status}<span class="file-status">{status}</span>{/if}
					</span>
				{/if}
			</li>
		{/each}
	</ul>
</section>
