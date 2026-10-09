/**
 * Escopo salvo por conexão no mock — a mesma sintaxe do `catalog::NameScope`
 * do backend: padrões separados por vírgula ou quebra de linha, `*` e `?`, `!`
 * na frente exclui, sem diferenciar maiúsculas.
 */
export function parseScope(rules: string | null | undefined) {
  const include: RegExp[] = [];
  const exclude: RegExp[] = [];

  for (const raw of (rules ?? '').split(/[,\n]/)) {
    const rule = raw.trim();
    const negated = rule.startsWith('!');
    const pattern = (negated ? rule.slice(1) : rule).trim();
    if (!pattern) continue;
    const source = pattern
      .replace(/[.+^${}()|[\]\\]/g, '\\$&')
      .replace(/\*/g, '.*')
      .replace(/\?/g, '.');
    (negated ? exclude : include).push(new RegExp(`^${source}$`, 'i'));
  }

  return (name: string) =>
    (include.length === 0 || include.some(re => re.test(name))) &&
    !exclude.some(re => re.test(name));
}
