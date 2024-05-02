module App = {
  @react.component
  let make = () => {
    <div> {React.string("Hello World")} </div>
  }
}

switch ReactDOM.querySelector("#app") {
| Some(domElement) => {
  Js.log(domElement);  
  ReactDOM.Client.createRoot(domElement)->ReactDOM.Client.Root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  )
}
| None => ()
}
