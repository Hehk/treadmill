type workout = {name: string}
type subscription = unit => unit
type bluetooth =
  | Off
  | Scanning
  | Connected

type t = {
  workouts: array<workout>,
  activeWorkout: option<workout>,
  subscriptions: Set.t<subscription>,
  bluetooth: bluetooth,
}

let state = ref({
  workouts: [{name: "6x400"}, {name: "10x3min"}],
  activeWorkout: None,
  subscriptions: Set.make(),
  bluetooth: Off,
})

type action =
  | WorkoutStart(workout)
  | WorkoutEnd
  | SubscriptionAdd(subscription)
  | SubscriptionRemove(subscription)

let reduce = (state, action) =>
  switch action {
  | WorkoutStart(workout) => {
      ...state,
      activeWorkout: Some(workout),
    }
  | WorkoutEnd => {
      ...state,
      activeWorkout: None,
    }
  | SubscriptionAdd(subscription) =>
    state.subscriptions->Set.add(subscription)
    state
  | SubscriptionRemove(subscription) =>
    state.subscriptions->Set.delete(subscription)->ignore
    state
  }

let update = action => {
  let newState = reduce(state.contents, action)

  if !Object.is(newState, state.contents) {
    state := newState
    newState.subscriptions->Set.forEach(s => s())
  }
}

let subscribe = subscription => {
  update(SubscriptionAdd(subscription))
  () => update(SubscriptionRemove(subscription))
}

let useGlobalState = selector =>
  // I wanted to play with this big boi
  React.useSyncExternalStore(~subscribe, ~getSnapshot=() => {
    selector(state.contents)
  })
