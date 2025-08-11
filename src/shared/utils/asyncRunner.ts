import { toast } from "react-hot-toast";

type RunnerDeps = {
  setIsLoading: (v: boolean) => void;
  setIsConnecting: (v: boolean) => void;
  setError: (v: string | null) => void;
};

type RunOpts<T> = {
  kind?: "load" | "connect";
  task: () => Promise<T>;
  onSuccess?: (r: T) => void;
  onError?: (e: unknown) => void;
};

export function createRunner({
  setIsLoading,
  setIsConnecting,
  setError,
}: RunnerDeps) {
  return async function run<T>({
    kind = "load",
    task,
    onSuccess,
    onError,
  }: RunOpts<T>): Promise<T> {
    if (kind === "connect") setIsConnecting(true);
    else setIsLoading(true);
    setError(null);

    try {
      const res = await task();
      onSuccess?.(res);
      return res;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      toast.error(msg);
      onError?.(e);
      throw e;
    } finally {
      if (kind === "connect") setIsConnecting(false);
      else setIsLoading(false);
    }
  };
}
