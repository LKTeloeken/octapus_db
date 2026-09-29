import octapusLogo from '@/assets/octapus.svg';
import { cn } from '@/lib/utils';

interface OctapusMarkProps {
  className?: string;
}

/**
 * Marca do Octapus: o polvo do logo (`assets/octapus.svg`) usado como máscara,
 * pintado com `bg-fg` — assim acompanha o tema (claro no escuro, escuro no
 * claro) sem precisar de duas imagens.
 */
export const OctapusMark = ({ className }: OctapusMarkProps) => (
  <span
    aria-hidden="true"
    className={cn('size-[22px] shrink-0 bg-fg', className)}
    style={{
      maskImage: `url(${octapusLogo})`,
      maskRepeat: 'no-repeat',
      maskPosition: 'center',
      maskSize: 'contain',
    }}
  />
);
