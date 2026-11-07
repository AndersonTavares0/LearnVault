import { tr } from './i18n.svelte';

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
		get name() { return tr('Wild Sapling', 'Brotinho Silvestre'); },
		get description() { return tr('Small resilient sprout. Ideal for quick blocks and focused reviews.', 'Pequena muda resiliente. Ideal para blocos rápidos e revisões pontuais.'); },
		cost: 0,
		minMinutes: 15,
		accentColor: '#65a30d',
		get lore() { return tr('Every ancient oak was once just a small sprout that refused to give up.', 'Todo carvalho ancestral um dia foi apenas um pequeno broto que se recusou a desistir.'); }
	},
	{
		id: 'oak',
		get name() { return tr('Leafy Oak', 'Carvalho Frondoso'); },
		get description() { return tr('The classic tree of discipline. Perfect balance between focus and consistency.', 'A clássica árvore da disciplina. Equilíbrio perfeito entre foco e consistência.'); },
		cost: 0,
		minMinutes: 25,
		accentColor: '#16a34a',
		get lore() { return tr('Symbol of perseverance and mental clarity in daily study.', 'Símbolo da perseverança e da clareza mental nos estudos diários.'); }
	},
	{
		id: 'pine',
		get name() { return tr('Noble Pine', 'Pinheiro Nobre'); },
		get description() { return tr('Slender and strong, withstands winters and deep study nights.', 'Esguio e forte, resiste a invernos e noites densas de estudo aprofundado.'); },
		cost: 40,
		minMinutes: 45,
		accentColor: '#0d9488',
		get lore() { return tr('Grows tall seeking the light of knowledge, unmoved by winds and storms.', 'Cresce alto buscando a luz do conhecimento, indiferente aos ventos e tempestades.'); }
	},
	{
		id: 'sakura',
		get name() { return tr('Blooming Cherry', 'Cerejeira em Flor'); },
		get description() { return tr('Spectacular and fleeting bloom. Crowns dedication with pink petals.', 'Florescimento espetacular e efêmero. Coroa a dedicação com pétalas rosadas.'); },
		cost: 80,
		minMinutes: 30,
		accentColor: '#ec4899',
		get lore() { return tr('Reminds us that learning is an act of delicate beauty and constant renewal.', 'Lembra que o aprendizado é um ato de delicada beleza e renovação constante.'); }
	},
	{
		id: 'willow',
		get name() { return tr('Weeping Willow', 'Salgueiro Chorão'); },
		get description() { return tr('Flexible branches dancing in the breeze. Represents adaptation and patience.', 'Galhos flexíveis que dançam com a brisa. Representa adaptação e paciência.'); },
		cost: 120,
		minMinutes: 40,
		accentColor: '#059669',
		get lore() { return tr('Flexible thinking overcomes any obstacle or difficult subject.', 'A flexibilidade de raciocínio supera qualquer obstáculo ou conteúdo difícil.'); }
	},
	{
		id: 'ginkgo',
		get name() { return tr('Golden Ginkgo', 'Ginkgo Dourado'); },
		get description() { return tr('Living fossil with golden fan-shaped leaves. Monument to lasting memory.', 'Fóssil vivo de folhas douradas em leque. Monumento à memória duradoura.'); },
		cost: 200,
		minMinutes: 50,
		accentColor: '#eab308',
		get lore() { return tr('Preserves ancient wisdom; its golden leaves symbolize deep retention.', 'Preserva sabedorias milenares; suas folhas douradas simbolizam a retenção profunda.'); }
	},
	{
		id: 'ancient',
		get name() { return tr('Ancient Tree', 'Árvore Ancestral'); },
		get description() { return tr('Titan of the forest with a golden aura and roots embracing the earth.', 'Titã da floresta com aura dourada e raízes que abraçam a terra inteira.'); },
		cost: 350,
		minMinutes: 60,
		accentColor: '#ca8a04',
		get lore() { return tr('The pinnacle of immersion and mastery. A living monument built by hours of uninterrupted dedication.', 'O ápice da imersão e maestria. Um monumento vivo erguido por horas de dedicação ininterrupta.'); }
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
	get 0() {
		return [
			tr('Your seed has taken root in fertile soil. Keep your eyes on the lesson!', 'Sua semente fincou raízes na terra fértil. Mantenha os olhos na aula!'),
			tr('Journey started. Resist the urge to switch windows!', 'Início de jornada. Resista à tentação de mudar de janela!'),
			tr('Focus begins in the first minute. Let your mind settle.', 'A concentração começa no primeiro minuto. Deixe a mente serenar.'),
			tr('A drop of discipline now nourishes a tree tomorrow.', 'Uma gota de disciplina agora nutre uma árvore amanhã.')
		];
	},
	get 1() {
		return [
			tr('The first sprout broke through the soil! Keep following the reasoning.', 'O primeiro broto rompeu o solo! Continue acompanhando o raciocínio.'),
			tr('Your roots are growing deeper. Do not get distracted by notifications.', 'Suas raízes estão se aprofundando. Não se distraia com notificações.'),
			tr('The content is sinking in. Every minute invested counts!', 'O conteúdo está se fixando. Cada minuto investido conta!'),
			tr('Steady stem and unwavering focus. Stay in the learning flow.', 'Caule firme e foco inabalável. Continue no fluxo de aprendizado.')
		];
	},
	get 2() {
		return [
			tr('Strong branches opening up! More than halfway there.', 'Galhos fortes se abrindo! Mais da metade do caminho percorrido.'),
			tr('The canopy is taking shape. Take a deep breath and keep the pace.', 'A copa está tomando forma. Respire fundo e mantenha o ritmo.'),
			tr('Excellent consistency! Distraction lost its turn to study.', 'Excelente consistência! A distração perdeu a vez para o estudo.'),
			tr('Your tree already stands out in the grove. Almost ready to bloom!', 'Sua árvore já se destaca no bosque. Falta pouco para o florescer!')
		];
	},
	get 3() {
		return [
			tr('Final stretch! The leaves are vibrant and ready to unfold.', 'Reta final! As folhas estão vibrantes e prontas para desabrochar.'),
			tr('Last minutes! Keep your immersion until the timer hits zero.', 'Últimos minutos! Mantenha a imersão até o relógio zerar.'),
			tr('Victory in sight! Your tree is about to complete its cycle.', 'Vitória à vista! Sua árvore está prestes a completar seu ciclo.'),
			tr('Almost there! Get ready to harvest your focus drops!', 'Quase lá! Prepare-se para colher suas gotas de foco!')
		];
	}
};

export function getGuardianPhrase(speciesId: string, phase: 0 | 1 | 2 | 3): string {
	const phrases = FOCUS_GUARDIAN_PHRASES[phase] ?? FOCUS_GUARDIAN_PHRASES[0];
	const species = getSpeciesById(speciesId);
	if (phase === 3) {
		return tr(
			`Almost there! Your ${species.name} is about to bloom!`,
			`Quase lá! Seu ${species.name} está prestes a florescer!`
		);
	}
	const index = (speciesId.length + phase) % phrases.length;
	return phrases[index];
}
