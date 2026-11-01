export const FOREST_NURSERY_STORAGE_KEY = 'learnvault-forest-nursery';
export const FOREST_SESSION_SEEDS_KEY = 'learnvault-forest-session-seeds';

export type BotanicalSpeciesId = 'sapling' | 'oak' | 'pine' | 'sakura' | 'willow' | 'ginkgo' | 'ancient';

export type BotanicalSpecies = {
	id: BotanicalSpeciesId;
	name: string;
	description: string;
	cost: number;
	minMinutes: number;
	accentColor: string;
	lore: string;
};

export const BOTANICAL_SPECIES: BotanicalSpecies[] = [
	{
		id: 'sapling',
		name: 'Brotinho Silvestre',
		description: 'Pequena muda resiliente. Ideal para blocos rápidos e revisões pontuais.',
		cost: 0,
		minMinutes: 15,
		accentColor: '#65a30d',
		lore: 'Todo carvalho ancestral um dia foi apenas um pequeno broto que se recusou a desistir.'
	},
	{
		id: 'oak',
		name: 'Carvalho Frondoso',
		description: 'A clássica árvore da disciplina. Equilíbrio perfeito entre foco e consistência.',
		cost: 0,
		minMinutes: 25,
		accentColor: '#16a34a',
		lore: 'Símbolo da perseverança e da clareza mental nos estudos diários.'
	},
	{
		id: 'pine',
		name: 'Pinheiro Nobre',
		description: 'Esguio e forte, resiste a invernos e noites densas de estudo aprofundado.',
		cost: 40,
		minMinutes: 45,
		accentColor: '#0d9488',
		lore: 'Cresce alto buscando a luz do conhecimento, indiferente aos ventos e tempestades.'
	},
	{
		id: 'sakura',
		name: 'Cerejeira em Flor',
		description: 'Florescimento espetacular e efêmero. Coroa a dedicação com pétalas rosadas.',
		cost: 80,
		minMinutes: 30,
		accentColor: '#ec4899',
		lore: 'Lembra que o aprendizado é um ato de delicada beleza e renovação constante.'
	},
	{
		id: 'willow',
		name: 'Salgueiro Chorão',
		description: 'Galhos flexíveis que dançam com a brisa. Representa adaptação e paciência.',
		cost: 120,
		minMinutes: 40,
		accentColor: '#059669',
		lore: 'A flexibilidade de raciocínio supera qualquer obstáculo ou conteúdo difícil.'
	},
	{
		id: 'ginkgo',
		name: 'Ginkgo Dourado',
		description: 'Fóssil vivo de folhas douradas em leque. Monumento à memória duradoura.',
		cost: 200,
		minMinutes: 50,
		accentColor: '#eab308',
		lore: 'Preserva sabedorias milenares; suas folhas douradas simbolizam a retenção profunda.'
	},
	{
		id: 'ancient',
		name: 'Árvore Ancestral',
		description: 'Titã da floresta com aura dourada e raízes que abraçam a terra inteira.',
		cost: 350,
		minMinutes: 60,
		accentColor: '#ca8a04',
		lore: 'O ápice da imersão e maestria. Um monumento vivo erguido por horas de dedicação ininterrupta.'
	}
];

export type NurseryState = {
	dropsBalance: number;
	totalDropsEarned: number;
	unlockedSpecies: BotanicalSpeciesId[];
	activeSeed: BotanicalSpeciesId;
};

export const DEFAULT_NURSERY_STATE: NurseryState = {
	dropsBalance: 0,
	totalDropsEarned: 0,
	unlockedSpecies: ['sapling', 'oak'],
	activeSeed: 'oak'
};

export function getSpeciesById(id: BotanicalSpeciesId | string): BotanicalSpecies {
	return BOTANICAL_SPECIES.find((s) => s.id === id) ?? BOTANICAL_SPECIES[1]; // default to oak
}

export function calculateDropsEarned(actualSeconds: number, completed: boolean): number {
	if (!completed || !Number.isFinite(actualSeconds) || actualSeconds <= 0) return 0;
	const minutes = Math.round(actualSeconds / 60);
	if (minutes <= 0) return 0;
	const calculated = Math.floor(minutes * 0.5);
	return Math.max(1, calculated);
}

export function calculateGrowthPhase(remainingSeconds: number, totalSeconds: number): 0 | 1 | 2 | 3 {
	if (totalSeconds <= 0) return 3;
	const remaining = Math.max(0, remainingSeconds);
	const elapsed = Math.max(0, totalSeconds - remaining);
	const ratio = Math.min(1, Math.max(0, elapsed / totalSeconds));

	if (ratio < 0.25) return 0;
	if (ratio < 0.5) return 1;
	if (ratio < 0.75) return 2;
	return 3;
}

export function canUnlockSpecies(
	balance: number,
	speciesId: BotanicalSpeciesId,
	unlocked: BotanicalSpeciesId[]
): boolean {
	if (unlocked.includes(speciesId)) return false;
	const species = getSpeciesById(speciesId);
	return balance >= species.cost;
}

export function unlockSpecies(
	state: NurseryState,
	speciesId: BotanicalSpeciesId
): { success: boolean; state: NurseryState } {
	if (!canUnlockSpecies(state.dropsBalance, speciesId, state.unlockedSpecies)) {
		return { success: false, state };
	}
	const species = getSpeciesById(speciesId);
	const newBalance = state.dropsBalance - species.cost;
	const newUnlocked = [...state.unlockedSpecies, speciesId];
	return {
		success: true,
		state: {
			...state,
			dropsBalance: newBalance,
			unlockedSpecies: newUnlocked,
			activeSeed: speciesId
		}
	};
}

export function selectActiveSeed(state: NurseryState, speciesId: BotanicalSpeciesId): NurseryState {
	if (!state.unlockedSpecies.includes(speciesId)) {
		return state;
	}
	return {
		...state,
		activeSeed: speciesId
	};
}

export function parseNurseryState(source: unknown): NurseryState {
	if (!source || typeof source !== 'object') {
		return { ...DEFAULT_NURSERY_STATE };
	}
	const data = source as Partial<NurseryState>;

	const dropsBalance =
		typeof data.dropsBalance === 'number' && Number.isFinite(data.dropsBalance) && data.dropsBalance >= 0
			? Math.floor(data.dropsBalance)
			: DEFAULT_NURSERY_STATE.dropsBalance;

	const totalDropsEarned =
		typeof data.totalDropsEarned === 'number' && Number.isFinite(data.totalDropsEarned) && data.totalDropsEarned >= 0
			? Math.floor(data.totalDropsEarned)
			: dropsBalance;

	const validIds: BotanicalSpeciesId[] = BOTANICAL_SPECIES.map((s) => s.id);
	const rawUnlocked = Array.isArray(data.unlockedSpecies) ? data.unlockedSpecies : [];
	const unlockedSpecies = rawUnlocked.filter((id): id is BotanicalSpeciesId => validIds.includes(id as BotanicalSpeciesId));

	// Always ensure base species are unlocked
	if (!unlockedSpecies.includes('sapling')) unlockedSpecies.unshift('sapling');
	if (!unlockedSpecies.includes('oak')) unlockedSpecies.push('oak');

	const activeSeed =
		typeof data.activeSeed === 'string' && unlockedSpecies.includes(data.activeSeed as BotanicalSpeciesId)
			? (data.activeSeed as BotanicalSpeciesId)
			: 'oak';

	return {
		dropsBalance,
		totalDropsEarned,
		unlockedSpecies,
		activeSeed
	};
}

export function readNurseryState(storage: Pick<Storage, 'getItem'> | null | undefined): NurseryState {
	try {
		const raw = storage?.getItem(FOREST_NURSERY_STORAGE_KEY);
		return raw ? parseNurseryState(JSON.parse(raw)) : { ...DEFAULT_NURSERY_STATE };
	} catch {
		return { ...DEFAULT_NURSERY_STATE };
	}
}

export function saveNurseryState(storage: Pick<Storage, 'setItem'> | null | undefined, state: NurseryState): void {
	try {
		storage?.setItem(FOREST_NURSERY_STORAGE_KEY, JSON.stringify(state));
	} catch {
		// Ignore storage quota errors
	}
}

export function readSessionSeeds(storage: Pick<Storage, 'getItem'> | null | undefined): Record<string, BotanicalSpeciesId> {
	try {
		const raw = storage?.getItem(FOREST_SESSION_SEEDS_KEY);
		if (!raw) return {};
		const parsed = JSON.parse(raw);
		if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
		const validIds: BotanicalSpeciesId[] = BOTANICAL_SPECIES.map((s) => s.id);
		const sanitized: Record<string, BotanicalSpeciesId> = {};
		for (const [key, value] of Object.entries(parsed)) {
			if (key === '__proto__' || key === 'constructor' || key === 'prototype') continue;
			if (typeof value === 'string' && validIds.includes(value as BotanicalSpeciesId)) {
				sanitized[key] = value as BotanicalSpeciesId;
			}
		}
		return sanitized;
	} catch {
		return {};
	}
}

export function recordSessionSeed(
	storage: Pick<Storage, 'getItem' | 'setItem'> | null | undefined,
	sessionId: string,
	speciesId: BotanicalSpeciesId
): void {
	if (!sessionId || sessionId === '__proto__' || sessionId === 'constructor' || sessionId === 'prototype') return;
	const validIds = BOTANICAL_SPECIES.map((s) => s.id);
	if (!validIds.includes(speciesId)) return;
	try {
		const current = readSessionSeeds(storage);
		current[sessionId] = speciesId;
		storage?.setItem(FOREST_SESSION_SEEDS_KEY, JSON.stringify(current));
	} catch {
		// Ignore storage quota errors
	}
}

export const FOCUS_GUARDIAN_PHRASES: Record<number, string[]> = {
	0: [
		'Sua semente fincou raízes na terra fértil. Mantenha os olhos na aula!',
		'Início de jornada. Resista à tentação de mudar de janela!',
		'A concentração começa no primeiro minuto. Deixe a mente serenar.',
		'Uma gota de disciplina agora nutre uma árvore amanhã.'
	],
	1: [
		'O primeiro broto rompeu o solo! Continue acompanhando o raciocínio.',
		'Suas raízes estão se aprofundando. Não se distraia com notificações.',
		'O conteúdo está se fixando. Cada minuto investido conta!',
		'Caule firme e foco inabalável. Continue no fluxo de aprendizado.'
	],
	2: [
		'Galhos fortes se abrindo! Mais da metade do caminho percorrido.',
		'A copa está tomando forma. Respire fundo e mantenha o ritmo.',
		'Excelente consistência! A distração perdeu a vez para o estudo.',
		'Sua árvore já se destaca no bosque. Falta pouco para o florescer!'
	],
	3: [
		'Reta final! As folhas estão vibrantes e prontas para desabrochar.',
		'Últimos minutos! Mantenha a imersão até o relógio zerar.',
		'Vitória à vista! Sua árvore está prestes a completar seu ciclo.',
		'Quase lá! Prepare-se para colher suas gotas de foco!'
	]
};

export function getGuardianPhrase(speciesId: string, phase: 0 | 1 | 2 | 3): string {
	const phrases = FOCUS_GUARDIAN_PHRASES[phase] ?? FOCUS_GUARDIAN_PHRASES[0];
	const species = getSpeciesById(speciesId);
	if (phase === 3) {
		return `Quase lá! Seu ${species.name} está prestes a florescer!`;
	}
	const index = (speciesId.length + phase) % phrases.length;
	return phrases[index];
}
