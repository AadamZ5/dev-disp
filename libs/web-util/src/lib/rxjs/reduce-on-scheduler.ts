import {
  Observable,
  OperatorFunction,
  SchedulerLike,
  Subscription,
} from 'rxjs';

const DEFAULT_REDUCER = <T, R>(prev: R, next: T) => next as unknown as R;

export function reduceOnScheduler<T>(
  scheduler: SchedulerLike,
): OperatorFunction<T, T>;
export function reduceOnScheduler<T, R>(
  scheduler: SchedulerLike,
  reducer: (prev: R, next: T) => R,
): OperatorFunction<T, R>;
export function reduceOnScheduler<T, R>(
  scheduler: SchedulerLike,
  reducer: (prev: R, next: T) => R = DEFAULT_REDUCER,
): OperatorFunction<T, R> {
  return (source) => {
    return new Observable<R>((subscriber) => {
      let lastValue: R | undefined;
      let completed = false;

      let actionSubscription: Subscription | undefined;

      const actionRuns = () => {
        subscriber.next(lastValue as R);
        actionSubscription = undefined;
        checkComplete();
      };

      const checkComplete = () => {
        if (completed && (!actionSubscription || actionSubscription.closed)) {
          subscriber.complete();
        }
      };

      const sourceSubscription = source.subscribe({
        next(value: T) {
          // Don't care if the lastValue is undefined, reducer should handle it
          lastValue = reducer(lastValue as R, value);
          if (!actionSubscription) {
            actionSubscription = scheduler.schedule(actionRuns);
          }
        },
        error(err) {
          subscriber.error(err);
        },
        complete() {
          completed = true;
          checkComplete();
        },
      });

      return new Subscription(() => {
        sourceSubscription.unsubscribe();
        actionSubscription?.unsubscribe();
      });
    });
  };
}
