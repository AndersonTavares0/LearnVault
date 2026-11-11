/** A one-page PDF with real text, built in memory so the tests need no binary file. */
export function makePdf(lines: string[]): Buffer {
	const stream = `BT /F1 20 Tf 40 240 Td ${lines.map((line, index) => `${index ? '0 -40 Td ' : ''}(${line.replace(/[()\\]/g, '\\$&')}) Tj`).join(' ')} ET`;
	const objects = [
		'<< /Type /Catalog /Pages 2 0 R >>',
		'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
		'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 420 300] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
		`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`,
		'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'
	];
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
