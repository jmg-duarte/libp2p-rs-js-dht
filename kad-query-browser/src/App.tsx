import { useEffect, useState } from "react";
import "./App.css";
import { perform_query, setup_logging } from "kad-query";

async function submit(bootnodes: string[], peerId: string) {
  await perform_query(bootnodes, peerId);
}

function App() {
  const [bootnodes, setBootnodes] = useState("");
  const [peerId, setPeerId] = useState("");

  useEffect(() => {
    setup_logging();
  });

  return (
    <>
      <div>
        <input
          type="text"
          placeholder="Bootnodes"
          onChange={(e) => setBootnodes(e.target.value)}
        />
        <input
          type="text"
          placeholder="PeerId"
          onChange={(e) => setPeerId(e.target.value)}
        />
        <input
          type="button"
          value="Submit"
          onClick={() => submit(bootnodes.split(","), peerId)}
        />
      </div>
    </>
  );
}

export default App;
