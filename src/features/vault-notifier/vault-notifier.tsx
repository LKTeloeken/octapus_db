import { useEffect } from 'react';
import toast from 'react-hot-toast';
import { Button } from '@/components/ui/button';
import { useVaultNotifier } from './use-vault-notifier';

const TOAST_ID = 'vault-broken';

/**
 * Avisa no boot quando o cofre não abre.
 *
 * Sem isso, o sintoma chegava disfarçado: toda senha decifrava para vazio e o
 * banco respondia "authentication failed", mandando o usuário conferir a
 * credencial no servidor errado.
 */
export function VaultNotifier() {
  const { isBroken } = useVaultNotifier();

  useEffect(() => {
    if (!isBroken) return;

    toast.custom(
      () => (
        <div className="flex min-w-72 max-w-96 flex-col gap-3 rounded-xl border border-white/10 bg-[rgba(18,18,18,0.5)] p-4 text-white shadow-[0_4px_20px_rgba(0,0,0,0.25)] backdrop-blur-md">
          <div>
            <p className="text-sm font-medium">Cofre de senhas indisponível</p>
            <p className="text-xs text-white/60">
              O arquivo <code>vault.key</code> está ausente, corrompido ou é de
              outra instalação. As senhas salvas não podem ser recuperadas —
              edite cada servidor e cadastre a senha de novo.
            </p>
          </div>
          <div className="flex justify-end">
            <Button
              variant="ghost"
              className="text-white hover:bg-white/10"
              onClick={() => toast.dismiss(TOAST_ID)}
            >
              Entendi
            </Button>
          </div>
        </div>
      ),
      { id: TOAST_ID, duration: Infinity, position: 'bottom-right' },
    );
  }, [isBroken]);

  return null;
}
