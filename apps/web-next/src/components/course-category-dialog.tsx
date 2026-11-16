import { useState, type FormEvent } from 'react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useStore } from '@/hooks/use-store';
import { courseCategoriesStore, resolveCourseCategory, setCourseCategory } from '@/lib/course-categories';
import { tr } from '@/lib/i18n';
import type { CatalogNode } from '@/lib/transport';
import { useCatalog } from '@/app/data';
import { safeStorage } from '@/app/preferences';

export function CourseCategoryDialog({
	course,
	courseId,
	open,
	onOpenChange
}: {
	course: Pick<CatalogNode, 'courseId' | 'path' | 'title' | 'sourceFolder'>;
	courseId: string;
	open: boolean;
	onOpenChange: (open: boolean) => void;
}) {
	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>{tr('Course category', 'Categoria do curso')}</DialogTitle>
					<DialogDescription>{course.title}</DialogDescription>
				</DialogHeader>
				{open && <CategoryForm course={course} courseId={courseId} onClose={() => onOpenChange(false)} />}
			</DialogContent>
		</Dialog>
	);
}

function CategoryForm({
	course,
	courseId,
	onClose
}: {
	course: Pick<CatalogNode, 'courseId' | 'path' | 'title' | 'sourceFolder'>;
	courseId: string;
	onClose: () => void;
}) {
	const catalog = useCatalog();
	const customCategories = useStore(courseCategoriesStore);
	const [draft, setDraft] = useState(() => resolveCourseCategory(course, customCategories) ?? '');

	const suggestions = [
		...new Set(
			(catalog.data?.nodes ?? [])
				.map((node) => resolveCourseCategory(node, customCategories))
				.filter((name): name is string => Boolean(name))
		)
	].sort((a, b) => a.localeCompare(b));

	const save = (event: FormEvent) => {
		event.preventDefault();
		setCourseCategory(courseId, draft.trim() || null, safeStorage());
		onClose();
	};

	const clear = () => {
		setCourseCategory(courseId, null, safeStorage());
		onClose();
	};

	return (
		<form onSubmit={save} className="space-y-4">
			<div className="space-y-2">
				<Label htmlFor={`category-input-${courseId}`}>{tr('Category name', 'Nome da categoria')}</Label>
				<Input
					id={`category-input-${courseId}`}
					value={draft}
					onChange={(event) => setDraft(event.target.value)}
					placeholder={tr('e.g. Backend, DevOps, Math', 'ex.: Backend, DevOps, Matemática')}
					maxLength={40}
				/>
			</div>
			{suggestions.length > 0 && (
				<div className="space-y-1.5">
					<p className="text-xs font-medium text-muted-foreground">{tr('Existing categories', 'Categorias existentes')}</p>
					<div className="flex flex-wrap gap-1.5">
						{suggestions.map((name) => (
							<Button key={name} type="button" size="xs" variant={draft.trim() === name ? 'selected' : 'outline'} onClick={() => setDraft(name)}>
								{name}
							</Button>
						))}
					</div>
				</div>
			)}
			<DialogFooter>
				{Boolean(customCategories[courseId]) && (
					<Button type="button" variant="ghost" onClick={clear} className="sm:mr-auto">
						{tr('Reset category', 'Limpar categoria')}
					</Button>
				)}
				<DialogClose render={<Button type="button" variant="outline" />}>{tr('Cancel', 'Cancelar')}</DialogClose>
				<Button type="submit">{tr('Save', 'Salvar')}</Button>
			</DialogFooter>
		</form>
	);
}
