/** Writes numbered objects, a cross-reference table and a trailer. Object 1 is the catalog. */
export function buildPdf(objects: string[]): Buffer {
	let body = '%PDF-1.4\n';
	const offsets: number[] = [];
	objects.forEach((object, index) => {
		offsets.push(body.length);
		body += `${index + 1} 0 obj\n${object}\nendobj\n`;
	});
	const xref = body.length;
	body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.map((offset) => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
	return Buffer.from(body, 'latin1');
}

const page = (stream: string) => [
	`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`,
	'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'
];

/** A one-page PDF with real text, built in memory so the tests need no binary file. */
export function makePdf(lines: string[]): Buffer {
	const stream = `BT /F1 20 Tf 40 240 Td ${lines.map((line, index) => `${index ? '0 -40 Td ' : ''}(${line.replace(/[()\\]/g, '\\$&')}) Tj`).join(' ')} ET`;
	return buildPdf([
		'<< /Type /Catalog /Pages 2 0 R >>',
		'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
		'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 420 300] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
		...page(stream)
	]);
}

/** Files that try to make a PDF viewer run code, follow a script link, loop, or break. Each must end as text or an error, nothing more. */
export function makeHostilePdfs(): Record<string, Buffer> {
	const stream = 'BT /F1 20 Tf 40 240 Td (Hostile) Tj ET';
	return {
		// An open action and a link that both try to run script in the page.
		script: buildPdf([
			'<< /Type /Catalog /Pages 2 0 R /OpenAction 6 0 R /AA << /O 6 0 R >> >>',
			'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
			'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 420 300] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Annots [7 0 R] >>',
			...page(stream),
			'<< /S /JavaScript /JS (window.parent.pdfpwned = 1; app.alert\\(1\\)) >>',
			'<< /Type /Annot /Subtype /Link /Rect [0 0 420 300] /Border [0 0 0] /A << /S /URI /URI (javascript:window.pdfpwned = 2) >> >>'
		]),
		// A page tree that contains itself and claims a huge page count.
		cycle: buildPdf([
			'<< /Type /Catalog /Pages 2 0 R >>',
			'<< /Type /Pages /Kids [2 0 R 3 0 R] /Count 100000000 >>',
			'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 420 300] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
			...page(stream)
		]),
		// A file cut off in the middle of an object.
		truncated: Buffer.from('%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Ver', 'latin1')
	};
}
