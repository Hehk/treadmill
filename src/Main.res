module App = {
  @react.component
  let make = () => {
    let workouts = State.useGlobalState(state => state.workouts)
    let activeWorkout = State.useGlobalState(state => state.activeWorkout)

    <div>
      {workouts
      ->Array.map(workout =>
        <div key={workout.name} onClick={_ => State.update(WorkoutStart(workout))}>
          {React.string(workout.name)}
        </div>
      )
      ->React.array}
      <h1> {React.string("Active workout")} </h1>
      {switch activeWorkout {
      | Some(workout) =>
        <div>
          {React.string(workout.name)}
          <button onClick={_ => State.update(WorkoutEnd)}> {"End"->React.string} </button>
        </div>
      | None => <div> {React.string("No active workout")} </div>
      }}
    </div>
  }
}

switch ReactDOM.querySelector("#app") {
| Some(domElement) => {
    Js.log(domElement)
    ReactDOM.Client.createRoot(domElement)->ReactDOM.Client.Root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    )
  }
| None => ()
}
