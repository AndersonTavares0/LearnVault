import { Download, Settings } from 'lucide-react';
import { createContext, useContext, useState, type ReactNode } from 'react';
import { useLocation } from 'react-router';
import { Button, buttonVariants } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { cn } from '@/lib/utils';
import { LOCALE_LABELS, LOCALES, setLocale, tr } from '@/lib/i18n';
import { studySyncSupported } from '@/lib/player';
import { MAX_WEEKLY_GOAL, MIN_WEEKLY_GOAL } from '@/lib/statistics';
import { transportKind } from '@/lib/transport';
import { useCatalog, useRewards, useSession, useSetWeeklyGoal } from '@/app/data';
import { PageHeader } from '@/app/shell';
import { AppearanceSettings } from './appearance';
import { AvatarSettings, CredentialsForm } from './account';
import { FocusSettings } from './focus';
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
	const active = picked.id && picked.hash === location.hash ? picked.id : location.hash.slice(1) || 'appearance';
	const setActive = (id: string) => setPicked({ hash: location.hash, id });

	const sections = [
		{ id: 'appearance', label: tr('Appearance', 'Aparência') },
		{ id: 'profile', label: tr('Profile', 'Perfil') },
		...(studySyncSupported ? [{ id: 'study', label: tr('Study', 'Estudo') }] : []),
		{ id: 'focus', label: tr('Focus timer', 'Timer de foco') },
		...(!server || admin ? [{ id: 'updates', label: tr('Updates', 'Atualizações') }] : []),
		...(server && admin ? [{ id: 'users', label: tr('Users', 'Usuários') }, { id: 'backup', label: 'Backup' }] : [])
	];

	return (
		<ActiveSection.Provider value={active}>
		<div className="lg:grid lg:grid-cols-[12rem_1fr] lg:gap-10">
			{/* Buttons, not `#id` links: the hash belongs to the router. */}
			<nav aria-label={tr('Settings sections', 'Seções dos ajustes')} className="mb-6 lg:sticky lg:top-24 lg:mb-0 lg:self-start">
				<ul className="flex gap-1 overflow-x-auto lg:block lg:space-y-1">
					{sections.map((section) => (
						<li key={section.id}>
							<button type="button" aria-current={active === section.id ? 'page' : undefined} onClick={() => setActive(section.id)} className={cn('min-h-11 w-full rounded-md px-3 text-left text-sm font-medium whitespace-nowrap hover:bg-muted', active === section.id ? 'bg-brand-tint text-foreground' : 'text-muted-foreground hover:text-foreground')}>
								{section.label}
							</button>
						</li>
					))}
				</ul>
			</nav>
			<div className="min-w-0 space-y-10">
			<PageHeader icon={Settings} title={tr('Settings', 'Ajustes')} />
			<Section id="appearance" title={tr('Appearance', 'Aparência')}>
				<AppearanceSettings />
				<Choice legend={tr('Language', 'Idioma')} options={LOCALES} value={locale} label={(value) => LOCALE_LABELS[value]} onChange={(value) => setLocale(value)} />
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
			{server && admin && (
				<>
					<Section id="users" title={tr('Accounts and course access', 'Contas e acesso a cursos')}>
						<UserSettings courses={catalog.data?.nodes ?? []} />
					</Section>
					<Section id="backup" title={tr('Backup', 'Backup')} description={tr('Download a consistent copy of the database (progress, notes, accounts), checked for integrity. Course files are not included.', 'Baixe uma cópia consistente do banco (progresso, notas, contas), verificada quanto à integridade. Os arquivos dos cursos não entram.')}>
						<a href="/api/admin/backup" download className={buttonVariants({ variant: 'outline' })}><Download />{tr('Download backup (.sqlite)', 'Baixar backup (.sqlite)')}</a>
					</Section>
				</>
			)}
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
	if (!rewards.data) return null;
	return <Choice legend={tr('Weekly goal, in study days', 'Meta semanal, em dias de estudo')} options={GOAL_DAYS} value={String(rewards.data.weeklyGoal)} label={(value) => value} onChange={(value) => setGoal.mutate(Number(value))} />;
}

function Section({ id, title, description, children }: { id: string; title: string; description?: string; children?: ReactNode }) {
	if (useContext(ActiveSection) !== id) return null;
	return (
		<section id={id} aria-labelledby={`${id}-heading`} className="space-y-4">
			<div className="space-y-1">
				<h2 id={`${id}-heading`} className="text-lg font-semibold">{title}</h2>
				{description && <p className="text-sm text-muted-foreground">{description}</p>}
			</div>
			{children}
		</section>
	);
}

function Choice<T extends string>({ legend, options, value, label, onChange }: { legend: string; options: readonly T[]; value: T; label: (value: T) => string; onChange: (value: T) => void }) {
	return (
		<fieldset className="space-y-2">
			<legend className="text-sm font-medium">{legend}</legend>
			<div className="flex flex-wrap gap-2">
				{options.map((option) => (
					<Button key={option} variant={option === value ? 'default' : 'outline'} aria-pressed={option === value} onClick={() => onChange(option)}>
						{label(option)}
					</Button>
				))}
			</div>
		</fieldset>
	);
}
