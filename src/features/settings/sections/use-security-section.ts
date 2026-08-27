import { useState } from 'react';
import toast from 'react-hot-toast';
import {
  useDisableMasterPassword,
  useEnableMasterPassword,
  useLockVault,
  useResetVault,
  useVaultStatus,
} from '@/queries/use-vault';

export const useSecuritySection = () => {
  const { data: status } = useVaultStatus();

  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [isResetOpen, setResetOpen] = useState(false);

  const enable = useEnableMasterPassword();
  const disable = useDisableMasterPassword();
  const { lock } = useLockVault();
  const { reset } = useResetVault();

  const hasMasterPassword = status?.hasMasterPassword ?? false;
  const isLocked = status?.locked ?? false;
  const isBusy = enable.isPending || disable.isPending;

  const clear = () => {
    setPassword('');
    setConfirm('');
  };

  const mismatch = confirm.length > 0 && password !== confirm;
  const canEnable = password.length >= 8 && password === confirm && !isBusy;
  const canDisable = password.length > 0 && !isBusy;

  const handleEnable = async () => {
    try {
      await enable.mutateAsync(password);
      clear();
      toast.success('Senha mestre ativada. Ela será pedida uma vez por sessão.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleDisable = async () => {
    try {
      await disable.mutateAsync(password);
      clear();
      toast.success('Senha mestre desativada.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleLock = async () => {
    try {
      await lock();
      toast.success('Cofre trancado.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleReset = async () => {
    try {
      await reset();
      clear();
      toast.success('Cofre recriado. Cadastre as senhas dos servidores de novo.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return {
    hasMasterPassword,
    isLocked,
    isBusy,
    password,
    confirm,
    mismatch,
    canEnable,
    canDisable,
    isResetOpen,
    setPassword,
    setConfirm,
    setResetOpen,
    handleEnable,
    handleDisable,
    handleLock,
    handleReset,
  };
};
