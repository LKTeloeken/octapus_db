/** Fuso do computador (`America/Sao_Paulo`); `UTC` se o sistema não disser */
export function localTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
}

/**
 * Nomes IANA que o sistema conhece, com o `UTC` e o fuso local garantidos (o
 * `supportedValuesOf` lista só os canônicos, e o local pode ser um apelido).
 */
export function timezoneNames(): string[] {
  const names = new Set<string>(['UTC', localTimezone()]);
  try {
    // Fora do `lib` do TS deste projeto, mas presente nas webviews do Tauri
    const intl = Intl as typeof Intl & {
      supportedValuesOf?: (key: 'timeZone') => string[];
    };
    for (const name of intl.supportedValuesOf?.('timeZone') ?? []) {
      names.add(name);
    }
  } catch {
    // Motor sem `supportedValuesOf`: ficam o UTC e o local
  }
  return Array.from(names).sort((a, b) => a.localeCompare(b));
}

/**
 * Diferença para o UTC neste instante, como `UTC−03:00` — só para orientar a
 * escolha: o horário de verão de cada data é o banco quem aplica.
 */
export function timezoneOffsetLabel(name: string, at: Date = new Date()) {
  try {
    const part = new Intl.DateTimeFormat('en-US', {
      timeZone: name,
      timeZoneName: 'longOffset',
    })
      .formatToParts(at)
      .find(item => item.type === 'timeZoneName')?.value;
    if (!part) return null;
    // `GMT-03:00` → `UTC−03:00`; `GMT` puro é o próprio UTC
    const offset = part.replace('GMT', '').replace('-', '−');
    return `UTC${offset || '±00:00'}`;
  } catch {
    return null;
  }
}
