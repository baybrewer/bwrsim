// Ordered job queue (plan C4 command path): each job starts only after the previous one
// settles, so a save_prepare issued after a click runs after every earlier command.

export type SerialQueue = <T>(job: () => Promise<T> | T) => Promise<T>;

export function createSerialQueue(): SerialQueue {
  let tail: Promise<unknown> = Promise.resolve();
  return <T>(job: () => Promise<T> | T): Promise<T> => {
    const run = tail.then(() => job());
    // A failed job must not block the jobs queued behind it.
    tail = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  };
}
