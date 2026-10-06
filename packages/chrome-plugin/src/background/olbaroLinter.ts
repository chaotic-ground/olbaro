import {
	type Dialect,
	type LintConfig,
	type StructuredLintConfig,
	type StructuredLintSetting,
	SuggestionKind,
} from 'harper.js';
import type { LintKind, UnpackedLint, UnpackedLintGroups } from 'lint-framework';
import init, { OlbaroLinter as WasmLinter } from '../olbaro-wasm/olbaro_wasm';
import wasmPath from '../olbaro-wasm/olbaro_wasm_bg.wasm?url';

/** A rule as `OlbaroLinter::rulesJson` in `crates/olbaro-wasm` reports it. */
type RuleInfo = {
	id: string;
	group: string;
	description: string;
	default_enabled: boolean;
	enabled: boolean;
};

/** A diagnostic as `OlbaroLinter::lintJson` reports it. Offsets are UTF-16, like JS strings. */
type OlbaroLint = {
	rule: string;
	group: string;
	severity: 'error' | 'warning' | 'hint';
	start: number;
	end: number;
	problem_text: string;
	message: string;
	/** Advice for the writer, not text to put in place of the span. */
	suggestion: string | null;
	/** Text to put in place of the span; each becomes a one-click fix. */
	replacements: string[];
};

const GROUP_LABELS: Record<string, string> = {
	llmstyle: 'LLM 말투',
};

const SEVERITY_KINDS: Record<OlbaroLint['severity'], { kind: LintKind; pretty: string }> = {
	error: { kind: 'Spelling', pretty: '오류' },
	warning: { kind: 'Style', pretty: '문체' },
	hint: { kind: 'Enhancement', pretty: '안내' },
};

let wasmReady: Promise<unknown> | undefined;

/**
 * Stands in for harper.js's `LocalLinter` in the background worker, backed by the olbaro WASM
 * build. It keeps the method names the worker already calls so that the worker's diff from
 * upstream Harper stays small.
 */
export default class OlbaroLinter {
	private readonly dialect: Dialect;
	private inner: WasmLinter | undefined;
	private setupDone: Promise<void> | undefined;
	private config: LintConfig = {};
	private ignored = new Set<string>();
	private words: string[] = [];

	constructor({ dialect }: { dialect: Dialect }) {
		this.dialect = dialect;
	}

	/** Loads the WASM module. Every other method waits for this, as harper.js's linters do. */
	setup(): Promise<void> {
		this.setupDone ??= (async () => {
			wasmReady ??= init({ module_or_path: chrome.runtime.getURL(wasmPath) });
			await wasmReady;
			this.rebuild();
		})();
		return this.setupDone;
	}

	dispose(): void {
		this.inner?.free();
		this.inner = undefined;
	}

	async getDialect(): Promise<Dialect> {
		return this.dialect;
	}

	async lint(text: string): Promise<UnpackedLintGroups> {
		const lints = JSON.parse((await this.linter()).lintJson(text)) as OlbaroLint[];
		const groups: UnpackedLintGroups = {};
		for (const lint of lints) {
			const unpacked = unpack(text, lint);
			if (this.ignored.has(unpacked.context_hash)) {
				continue;
			}
			(groups[lint.rule] ??= []).push(unpacked);
		}
		return groups;
	}

	async setLintConfig(config: LintConfig): Promise<void> {
		this.config = { ...config };
		if (this.inner != null) {
			this.rebuild();
		}
	}

	async getLintConfigAsJSON(): Promise<string> {
		return JSON.stringify(this.config);
	}

	async getStructuredLintConfigJSON(): Promise<string> {
		const groups = new Map<string, RuleInfo[]>();
		for (const rule of await this.rules()) {
			const rules = groups.get(rule.group) ?? [];
			rules.push(rule);
			groups.set(rule.group, rules);
		}
		const settings: StructuredLintSetting[] = [...groups].map(([group, rules]) => ({
			Group: {
				label: GROUP_LABELS[group] ?? group,
				description: '',
				child: {
					settings: rules.map((rule) => ({
						Bool: { name: rule.id, state: rule.enabled, label: rule.description },
					})),
				},
			},
		}));
		return JSON.stringify({ settings } satisfies StructuredLintConfig);
	}

	async getLintDescriptionsHTML(): Promise<Record<string, string>> {
		return Object.fromEntries((await this.rules()).map((rule) => [rule.id, escapeHtml(rule.description)]));
	}

	async ignoreLintHash(hash: bigint): Promise<void> {
		this.ignored.add(hash.toString());
	}

	async exportIgnoredLints(): Promise<string> {
		return JSON.stringify([...this.ignored]);
	}

	async importIgnoredLints(state: string): Promise<void> {
		try {
			const parsed: unknown = JSON.parse(state);
			if (Array.isArray(parsed)) {
				this.ignored = new Set(parsed.map(String));
			}
		} catch {
			// Harper's own format from an earlier install; start over.
			this.ignored = new Set();
		}
	}

	/** Kept so the options page can still edit the list; no olbaro rule reads it yet. */
	async importWords(words: string[]): Promise<void> {
		if (Array.isArray(words)) {
			this.words = [...new Set([...this.words, ...words])];
		}
	}

	async exportWords(): Promise<string[]> {
		return [...this.words];
	}

	async loadWeirpackFromBytes(_bytes: Uint8Array): Promise<undefined> {
		throw new Error('Weirpacks are Harper rules and do not run in olbaro.');
	}

	private async linter(): Promise<WasmLinter> {
		await this.setup();
		if (this.inner == null) {
			throw new Error('OlbaroLinter was used after dispose().');
		}
		return this.inner;
	}

	private rebuild(): void {
		this.inner?.free();
		this.inner = new WasmLinter(JSON.stringify(this.config));
	}

	private async rules(): Promise<RuleInfo[]> {
		return JSON.parse((await this.linter()).rulesJson()) as RuleInfo[];
	}
}

function unpack(text: string, lint: OlbaroLint): UnpackedLint {
	const { kind, pretty } = SEVERITY_KINDS[lint.severity];
	return {
		span: { start: lint.start, end: lint.end },
		// olbaro's suggestion is advice ("'바로'를 뺀다"), not replacement text, so it goes in the
		// message; one-click fixes come from `replacements`.
		message_html:
			lint.suggestion == null
				? escapeHtml(lint.message)
				: `${escapeHtml(lint.message)}<br>${escapeHtml(lint.suggestion)}`,
		problem_text: lint.problem_text,
		lint_kind: kind,
		lint_kind_pretty: pretty,
		suggestions: lint.replacements.map((replacement) => ({
			kind: SuggestionKind.Replace,
			replacement_text: replacement,
		})),
		context_hash: contextHash(text, lint),
		source: text,
	};
}

/**
 * Identifies a lint for "ignore" across edits elsewhere in the text: the rule, the flagged text
 * and a few characters on each side, hashed with 64-bit FNV-1a.
 */
function contextHash(text: string, lint: OlbaroLint): string {
	const context = `${lint.rule}\u0000${text.slice(Math.max(0, lint.start - 8), lint.end + 8)}`;
	let hash = 0xcbf29ce484222325n;
	for (let i = 0; i < context.length; i++) {
		hash ^= BigInt(context.charCodeAt(i));
		hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
	}
	return hash.toString();
}

function escapeHtml(text: string): string {
	return text
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;');
}
