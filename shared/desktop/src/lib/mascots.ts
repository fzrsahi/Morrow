// Source → mascot routing, ported verbatim from macOS:
//   SessionSnapshot.normalizedSupportedSource (MorrowCore/SessionSnapshot.swift)
//   MascotView.body                          (Morrow/MascotView.swift)
// Keys are the exported sprite names (the README GIF names, e.g. `factory` is
// DroidView and `claude` is ClawdView). A source we don't know renders Clawd,
// exactly like MascotView's `default` branch.

export type MascotStatus = 'idle' | 'working' | 'waitingApproval' | 'waitingQuestion';

export const MASCOT_SIZES = { collapsed: 21, card: 28, compact: 15 } as const;

export type MascotAsset =
  | 'aiwork'
  | 'antigravity'
  | 'claude'
  | 'cline'
  | 'codebuddy'
  | 'codex'
  | 'copilot'
  | 'cursor'
  | 'factory'
  | 'gemini'
  | 'grok'
  | 'hermes'
  | 'kimi'
  | 'kiro'
  | 'mimo'
  | 'minimax'
  | 'openclaw'
  | 'opencode'
  | 'pi'
  | 'qoder'
  | 'qwen'
  | 'stepfun'
  | 'trae'
  | 'workbuddy';

/** Spelling variants from SessionSnapshot.normalizedSupportedSource. */
const SOURCE_ALIASES: Record<string, string> = {
  factory: 'droid',
  ag: 'antigravity',
  'anti-gravity': 'antigravity',
  'anti gravity': 'antigravity',
  googleantigravity: 'google-antigravity',
  'google antigravity': 'google-antigravity',
  'google-anti-gravity': 'google-antigravity',
  'antigravity-ide': 'google-antigravity',
  'antigravity-cli': 'google-antigravity',
  agy: 'google-antigravity',
  'work-buddy': 'workbuddy',
  'work body': 'workbuddy',
  'work-body': 'workbuddy',
  workbody: 'workbuddy',
  'hermes-agent': 'hermes',
  'hermes-agents': 'hermes',
  'hermes agent': 'hermes',
  'hermes agents': 'hermes',
  'grok-cli': 'grok',
  grokcli: 'grok',
  'grok-build': 'grok',
  'grok build': 'grok',
  'open-claw': 'openclaw',
  'open claw': 'openclaw',
  clawdbot: 'openclaw',
  'qwen-code': 'qwen',
  qwencode: 'qwen',
  'cursor-agent': 'cursor-cli',
  cursoragent: 'cursor-cli',
  cursorcli: 'cursor-cli',
  qodercli: 'qoder-cli',
  qoderclicn: 'qoder-cli',
  'qodercli-cn': 'qoder-cli',
  'qoder-cn': 'qoder-cli',
  dtcoder: 'aiwork',
  'dtcoder-cli': 'aiwork-cli',
  dtcodercli: 'aiwork-cli',
  aiworkcli: 'aiwork-cli',
  qodercn: 'qoder-cli',
  'qoder-work': 'qoderwork',
  'qoder work': 'qoderwork',
  'kimi-cli': 'kimi',
  kimicli: 'kimi',
  'kimi-code': 'kimi',
  kimicode: 'kimi',
  kimi_code: 'kimi',
  'kimi-code-cli': 'kimi',
  kimicodecli: 'kimi',
  kimi_code_cli: 'kimi',
  'kiro-cli': 'kiro',
  kirocli: 'kiro',
  codebuddycn: 'codybuddycn',
  'codybuddy-cn': 'codybuddycn',
  'step-fun': 'stepfun',
  'step fun': 'stepfun',
  'trae-cn': 'traecn',
  trae_cn: 'traecn',
  'trae cn': 'traecn',
  omp: 'pi',
  'oh-my-pi': 'pi',
  'oh my pi': 'pi',
  'z-code': 'zcode',
  'z code': 'zcode',
  mcode: 'minimax',
  'minimax-code': 'minimax',
  minimaxcode: 'minimax',
  'minimax-cli': 'minimax',
  minimaxcli: 'minimax',
  'minimax-code-cli': 'minimax',
  mimocode: 'mimo',
  'mimo-code': 'mimo',
  mimo_code: 'mimo',
  'mimo code': 'mimo',
  'mimo-cli': 'mimo',
  mimocli: 'mimo',
  'mimo-desktop': 'mimo',
  mimodesktop: 'mimo',
  'xiaomi-mimo': 'mimo',
  xiaomimimo: 'mimo',
  'xiaomi mimo': 'mimo',
  'xiaomi-mimo-ai': 'mimo',
};

/** MascotView's switch, keyed by the *normalized* source. */
const MASCOT_ROUTES: Record<string, MascotAsset> = {
  codex: 'codex',
  grok: 'grok',
  gemini: 'gemini',
  'google-antigravity': 'gemini',
  cursor: 'cursor',
  'cursor-cli': 'cursor',
  trae: 'trae',
  traecn: 'trae',
  traecli: 'trae',
  copilot: 'copilot',
  qoder: 'qoder',
  'qoder-cli': 'qoder',
  qoderwork: 'qoder',
  droid: 'factory',
  codebuddy: 'codebuddy',
  codybuddycn: 'codebuddy',
  stepfun: 'stepfun',
  opencode: 'opencode',
  mimo: 'mimo',
  antigravity: 'antigravity',
  workbuddy: 'workbuddy',
  hermes: 'hermes',
  openclaw: 'openclaw',
  kiro: 'kiro',
  kimi: 'kimi',
  pi: 'pi',
  cline: 'cline',
  aiwork: 'aiwork',
  'aiwork-cli': 'aiwork',
  minimax: 'minimax',
};

/** The prefix fallbacks at the end of normalizedSupportedSource. */
const SOURCE_PREFIXES: Array<[string, string]> = [
  ['google-antigravity', 'google-antigravity'],
  ['googleantigravity', 'google-antigravity'],
  ['antigravity', 'antigravity'],
  ['workbuddy', 'workbuddy'],
  ['hermes', 'hermes'],
  ['grok', 'grok'],
  ['openclaw', 'openclaw'],
  ['qwen', 'qwen'],
  ['kiro', 'kiro'],
  ['kimi', 'kimi'],
  ['minimax', 'minimax'],
  ['codybuddycn', 'codybuddycn'],
  ['codebuddycn', 'codybuddycn'],
  ['stepfun', 'stepfun'],
  ['traecn', 'traecn'],
  ['traecli', 'traecli'],
  ['trae', 'trae'],
];

export function normalizeSource(source: string | null | undefined): string | null {
  if (source == null) return null;
  const trimmed = source.trim().toLowerCase();
  if (!trimmed) return null;
  const canonical = SOURCE_ALIASES[trimmed] ?? trimmed;
  if (canonical in MASCOT_ROUTES || canonical === 'claude') return canonical;
  for (const [prefix, target] of SOURCE_PREFIXES) {
    if (canonical.startsWith(prefix)) return target;
  }
  return null;
}

/** Resolves any CLI source to the sprite sheet name; unknown → Clawd. */
export function resolveMascot(source: string | null | undefined): MascotAsset {
  const canonical = normalizeSource(source);
  if (!canonical) return 'claude';
  return MASCOT_ROUTES[canonical] ?? 'claude';
}
