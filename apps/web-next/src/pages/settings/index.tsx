import { BookOpen, Check, Cloud, Database, Download, Flame, Info, Palette, RefreshCw, Server, Settings, Timer, UserRound, Users } from 'lucide-react';
import { createContext, useContext, useState, type ReactNode } from 'react';
import { useLocation } from 'react-router';
import { Panel } from '@/components/panel';
import { Button, buttonVariants } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { cn } from '@/lib/utils';
import { getLocale, LOCALE_LABELS, LOCALES, setLocale, tr } from '@/lib/i18n';
import { studySyncSupported } from '@/lib/player';
import { MAX_WEEKLY_GOAL, MIN_WEEKLY_GOAL } from '@/lib/statistics';
import { transportKind } from '@/lib/transport';
import { useCatalog, useRewards, useSession, useSetWeeklyGoal } from '@/app/data';
import { PageHeader } from '@/app/shell';
import { AboutSettings } from './about';
import { AppearanceSettings } from './appearance';
import { AvatarSettings, CredentialsForm } from './account';
import { FocusSettings } from './focus';
import { SourceSettings } from './sources';
import { SyncSettings } from './sync';
import { UpdateSettings } from './updates';
import { UserSettings } from './users';



const ActiveSection = createContext('appearance');

export function SettingsPage() {
	const locale = useLocale();
	const session = useSession();
	const catalog = useCatalog();
	const location = useLocation();
	const user = session.data?.user;
	const server = transportKind === 'http';
	const admin = Boolean(user?.isAdmin);

	// One section at a time keeps the page short. `#/settings#focus` (from the Pomodoro dialog) opens that section.
	const [picked, setPicked] = useState({ hash: location.hash, id: '' });
	const setActive = (id: string) => setPicked({ hash: location.hash, id });

	const sections = [
		{ id: 'appearance', label: tr('Appearance', 'Aparência'), icon: Palette },
		{ id: 'profile', label: tr('Profile', 'Perfil'), icon: UserRound },
		...(studySyncSupported ? [{ id: 'study', label: tr('Study', 'Estudo'), icon: BookOpen }] : []),
		{ id: 'focus', label: tr('Focus timer', 'Timer de foco'), icon: Timer },
		...(!server || admin ? [{ id: 'updates', label: tr('Updates', 'Atualizações'), icon: RefreshCw }] : []),
		...(!server ? [{ id: 'sync', label: tr('Sync', 'Sincronização'), icon: Cloud }] : []),
		...(server && admin ? [{ id: 'users', label: tr('Users', 'Usuários'), icon: Users }, { id: 'sources', label: tr('Remote sources', 'Fontes remotas'), icon: Server }, { id: 'backup', label: tr('Backup', 'Backup'), icon: Database }] : []),
		{ id: 'about', label: tr('About', 'Sobre'), icon: Info }
	];
	const requested = picked.id && picked.hash === location.hash ? picked.id : location.hash.slice(1) || 'appearance';
	const active = sections.some((section) => section.id === requested) ? requested : 'appearance';

	return (
		<ActiveSection.Provider value={active}>
		<div className="space-y-8">
		<PageHeader icon={Settings} title={tr('Settings', 'Ajustes')} />
		<div className="lg:grid lg:grid-cols-[12rem_1fr] lg:gap-10">
			{/* Buttons, not `#id` links: the hash belongs to the router. */}
			<nav aria-label={tr('Settings sections', 'Seções dos ajustes')} className="mb-6 lg:sticky lg:top-6 lg:mb-0 lg:self-start">
				<div className="space-y-2 lg:hidden">
					<label htmlFor="settings-section" className="block text-sm font-medium">{tr('Settings section', 'Seção dos ajustes')}</label>
					<select id="settings-section" value={active} onChange={(event) => setActive(event.target.value)} className="h-11 w-full rounded-lg border border-input bg-background px-3 text-sm">
						{sections.map((section) => <option key={section.id} value={section.id}>{section.label}</option>)}
					</select>
				</div>
				<ul className="hidden lg:block lg:space-y-1">
					{sections.map((section) => (
						<li key={section.id}>
							<button type="button" aria-current={active === section.id ? 'page' : undefined} onClick={() => setActive(section.id)} className={cn('flex min-h-11 w-full items-center gap-2 rounded-lg px-3 text-left text-sm font-medium whitespace-nowrap transition-colors hover:bg-muted', active === section.id ? 'bg-brand-tint font-semibold text-brand-ink' : 'text-muted-foreground hover:text-foreground')}>
								<section.icon className="size-4 shrink-0" aria-hidden="true" />
								{section.label}
							</button>
						</li>
					))}
				</ul>
			</nav>
			<div className="min-w-0 space-y-10">
			<Section id="appearance" title={tr('Appearance', 'Aparência')} framed={false}>
				<AppearanceSettings />
				<Panel className="flex flex-col gap-4 p-5 sm:flex-row sm:items-center sm:justify-between sm:p-6">
					<div className="space-y-1">
						<h3 id="language-heading" className="text-base font-bold">{tr('Language', 'Idioma')}</h3>
						<p className="text-sm text-muted-foreground">{tr('Menus, study dates, and exports follow your choice.', 'Menus, datas de estudo e exportações seguem este idioma.')}</p>
					</div>
					<div role="group" aria-labelledby="language-heading" className="flex flex-wrap gap-2">
						{LOCALES.map((value) => (
							<Button key={value} variant={value === locale ? 'selected' : 'outline'} aria-pressed={value === locale} onClick={() => setLocale(value)}>
								{LOCALE_LABELS[value]}
								{value === locale && <Check className="size-3.5 text-brand-ink" aria-hidden="true" />}
							</Button>
						))}
					</div>
				</Panel>
			</Section>
			{server && user && (
				<Section id="profile" title={tr('Profile', 'Perfil')} description={tr(`Signed in as ${user.username}${admin ? ' (administrator)' : ''}.`, `Conectado como ${user.username}${admin ? ' (administrador)' : ''}.`)}>
					<AvatarSettings user={user} />
					<h3 className="pt-2 font-medium">{tr('Sign-in details', 'Dados de acesso')}</h3>
					<CredentialsForm user={user} />
				</Section>
			)}
			{!server && <Section id="profile" title={tr('Profile', 'Perfil')} description={tr('Local library on this device. There is no account to sign in to.', 'Biblioteca local neste dispositivo. Não há conta para entrar.')} />}
			{studySyncSupported && (
				<Section id="study" title={tr('Study', 'Estudo')} description={tr('A missed week never resets your badges.', 'Uma semana sem estudar nunca zera seus emblemas.')}>
					<WeeklyGoal />
				</Section>
			)}
			<Section id="focus" title={tr('Focus timer', 'Timer de foco')}>
				<FocusSettings />
			</Section>
			{(!server || admin) && (
				<Section id="updates" title={tr('Updates', 'Atualizações')}>
					<UpdateSettings />
				</Section>
			)}
			{!server && (
				<Section id="sync" title={tr('Sync', 'Sincronização')} description={tr('Keep progress, notes and flashcards in step with a LearnVault server. Changes made offline wait on this device until the next sync.', 'Mantenha progresso, notas e flashcards em dia com um servidor LearnVault. Alterações feitas sem conexão esperam neste dispositivo até a próxima sincronização.')}>
					<SyncSettings />
				</Section>
			)}
			{server && admin && (
				<>
					<Section id="users" title={tr('Accounts and course access', 'Contas e acesso a cursos')}>
						<UserSettings courses={catalog.data?.nodes ?? []} />
					</Section>
					<Section id="sources" title={tr('Remote sources', 'Fontes remotas')} description={tr('Read-only WebDAV or SMB folders outside this machine.', 'Pastas WebDAV ou SMB somente leitura fora desta máquina.')}>
						<SourceSettings />
					</Section>
					<Section id="backup" title={tr('Backup', 'Backup')} description={tr('Download a consistent copy of the database (progress, notes, accounts), checked for integrity. Course files are not included.', 'Baixe uma cópia consistente do banco (progresso, notas, contas), verificada quanto à integridade. Os arquivos dos cursos não entram.')}>
						<a href="/api/admin/backup" download className={buttonVariants({ variant: 'outline' })}><Download />{tr('Download backup (.sqlite)', 'Baixar backup (.sqlite)')}</a>
					</Section>
				</>
			)}
			<Section id="about" title={tr('About', 'Sobre')}>
				<AboutSettings />
			</Section>
			</div>
		</div>
		</div>
		</ActiveSection.Provider>
	);
}

const GOAL_DAYS = Array.from({ length: MAX_WEEKLY_GOAL - MIN_WEEKLY_GOAL + 1 }, (_, index) => String(index + MIN_WEEKLY_GOAL));

/** Study days per week; Progress shows this goal against the week. */
function WeeklyGoal() {
	const rewards = useRewards();
	const setGoal = useSetWeeklyGoal();
	if (rewards.isPending && !rewards.data) return <p role="status" className="text-sm text-muted-foreground">{tr('Loading weekly goal…', 'Carregando meta semanal…')}</p>;
	if (rewards.error && !rewards.data) return <p role="alert" className="text-sm text-destructive">{rewards.error.message}</p>;
	if (!rewards.data) return null;
	const goal = rewards.data.weeklyGoal;
	const study = rewards.data.study;
	const activeThisWeek = Math.min(study.weekActiveDays, goal);
	const pace = goal <= 2
		? tr('Light cadence — great for busy weeks.', 'Ritmo leve — ideal para semanas corridas.')
		: goal <= 4
			? tr('Balanced cadence — keeps momentum steady.', 'Ritmo equilibrado — mantém a constância.')
			: tr('High consistency — builds a daily study habit.', 'Alta consistência — cria hábito diário.');
	const dayInitial = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'narrow' });
	return (
		<div className="grid gap-6 pt-1 lg:grid-cols-[1.25fr_1fr] lg:items-center">
			<div className="space-y-3">
				<Choice legend={tr('Weekly goal, in study days', 'Meta semanal, em dias de estudo')} options={GOAL_DAYS} value={String(goal)} label={(value) => value} disabled={setGoal.isPending} onChange={(value) => setGoal.mutate(Number(value))} />
				<p className="text-xs text-muted-foreground">{pace}</p>
				{setGoal.isError && <p role="alert" className="text-sm text-destructive">{setGoal.error instanceof Error ? setGoal.error.message : tr('Could not save weekly goal.', 'Não foi possível salvar a meta semanal.')}</p>}
				{setGoal.isSuccess && <p role="status" className="text-sm font-medium text-primary">{tr(`Weekly goal set to ${setGoal.data} study days.`, `Meta semanal definida para ${setGoal.data} dias de estudo.`)}</p>}
			</div>
			<div className="space-y-3.5 rounded-2xl border bg-panel/60 p-4">
				<div className="flex items-center justify-between gap-2">
					<div>
						<p className="text-xs font-medium text-muted-foreground">{tr('This week', 'Nesta semana')}</p>
						<p className="font-heading text-2xl font-bold tabular-nums">{activeThisWeek}/{goal} <span className="font-sans text-sm font-medium text-muted-foreground">{tr('days', 'dias')}</span></p>
					</div>
					<span className="inline-flex items-center gap-1.5 rounded-full bg-reward-tint px-3 py-1 text-xs font-semibold text-reward">
						<Flame className="size-3.5" aria-hidden="true" />
						{tr(`${study.currentStreak}d streak`, `${study.currentStreak}d seguidos`)}
					</span>
				</div>
				<div aria-hidden="true" className="flex gap-1.5">
					{Array.from({ length: goal }, (_, index) => (
						<span key={index} className={cn('h-1.5 flex-1 rounded-full transition-colors', index < activeThisWeek ? 'bg-primary' : 'bg-border')} />
					))}
				</div>
				{study.days.length > 0 && (
					<div aria-hidden="true" className="flex justify-between gap-1 pt-0.5">
						{study.days.map((day) => (
							<span key={day.day} className="flex flex-col items-center gap-1">
								<span className={cn('size-2.5 rounded-full', day.active ? 'bg-primary' : 'border border-border-strong bg-background')} />
								<span className="text-[11px] text-muted-foreground">{dayInitial(day.day)}</span>
							</span>
						))}
					</div>
				)}
			</div>
		</div>
	);
}

function Section({ id, title, description, children, framed = true }: { id: string; title: string; description?: string; children?: ReactNode; framed?: boolean }) {
	if (useContext(ActiveSection) !== id) return null;
	const heading = (
		<div className="space-y-1">
			<h2 id={`${id}-heading`} className="text-lg font-semibold">{title}</h2>
			{description && <p className="text-sm text-muted-foreground">{description}</p>}
		</div>
	);
	return (
		<section id={id} aria-labelledby={`${id}-heading`}>
			{framed ? <Panel className="space-y-4">{heading}{children}</Panel> : <div className="space-y-4">{heading}{children}</div>}
		</section>
	);
}

function Choice<T extends string>({ legend, options, value, label, disabled = false, onChange }: { legend: string; options: readonly T[]; value: T; label: (value: T) => string; disabled?: boolean; onChange: (value: T) => void }) {
	return (
		<fieldset className="min-w-0 space-y-2">
			<legend className="text-sm font-medium">{legend}</legend>
			<div className="flex flex-wrap gap-2">
				{options.map((option) => (
					<Button key={option} variant={option === value ? 'selected' : 'outline'} aria-pressed={option === value} disabled={disabled} onClick={() => onChange(option)}>
						{label(option)}
						{option === value && <Check className="size-3.5 text-primary" aria-hidden="true" />}
					</Button>
				))}
			</div>
		</fieldset>
	);
}
