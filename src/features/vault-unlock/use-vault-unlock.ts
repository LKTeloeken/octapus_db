import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { useResetVault, useUnlockVault, useVaultStatus } from '@/queries/use-vault';
import { useVaultStore } from '@/stores/vault-store';

export const useVaultUnlock = () => {
  const isOpen = useVaultStore(state => state.isUnlockOpen);
  const close = useVaultStore(state => state.closeUnlock);

  const { data: status } = useVaultStatus();
  const unlockVault = useUnlockVault();
  const { reset } = useResetVault();

  const [password, setPassword] = useState('');
  const [isResetOpen, setResetOpen] = useState(false);

  useEffect(() => {
    if (isOpen) setPassword('');
  }, [isOpen]);

  // Destravou por outro caminho (ou o cofre nem tem senha): não faz sentido
  // continuar pedindo.
  useEffect(() => {
    if (isOpen && status && !status.locked) close();
  }, [isOpen, status, close]);

  const handleUnlock = async () => {
    try {
      await unlockVault.unlock(password);
      setPassword('');
      close();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleReset = async () => {
    try {
      await reset();
      close();
      toast.success('Cofre recriado. Cadastre as senhas dos servidores de novo.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return {
    isOpen,
    close,
    password,
    setPassword,
    isSubmitting: unlockVault.isPending,
    canSubmit: password.length > 0 && !unlockVault.isPending,
    isResetOpen,
    setResetOpen,
    handleUnlock,
    handleReset,
  };
};
