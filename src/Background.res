// All code running at the Rust level, is called the Background.
// Ideally, all it's state is managed in the Foreground and communicated here with messages

let toResult = async (f, x) =>
  switch await f(x) {
  | value => Ok(value)
  | exception Js.Exn.Error(e) => Error(e)
  }

%%raw(`
import { invoke } from '@tauri-apps/api/tauri'

function readWorkouts() {
  return invoke('read_workouts')
}

function connectToTreadmill() {
  return invoke('connect_to_treadmill')
}
`)

let _readWorkouts: unit => promise<array<string>> = %raw(`
  function readWorkouts() {
    return invoke('read_workouts')
  }
`)
let readWorkouts = toResult(_readWorkouts, ...)
