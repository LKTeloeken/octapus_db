import { useEffect, useState } from 'react';

/** Segundos que faltam até `until` (epoch ms), atualizados a cada segundo */
export function useCountdown(until: number | null): number | null {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (until === null) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [until]);

  if (until === null) return null;
  return Math.max(0, Math.ceil((until - now) / 1000));
}
