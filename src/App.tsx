import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { format } from 'date-fns';
import './App.css';

type Connection = {
  date: string;
  earliest: string;
  latest: string;
};

const App = () => {
  const [connections, setConnections] = useState<Connection[]>([]);

  useEffect(() => {
    const fetchConnections = async () => {
      try {
        const data: Connection[] = await invoke('get_connections');
        console.log(data);
        setConnections(data);
      } catch (error) {
        console.error('Failed to fetch connections:', error);
      }
    };

    fetchConnections();
    const interval = setInterval(fetchConnections, 30000); // Update every 30s

    return () => clearInterval(interval);
  }, []);

  const formatDate = (date: string) => {
    return format(new Date(date), 'EEE MMM do').toUpperCase();
  };

  return (
    <div className="mx-auto p-4">
      <h1 className="text-2xl font-bold mb-4 merriweather-black">Eduroam Connection Log</h1>
      <ul className="space-y-2">
        {connections.map((conn, index) => (
          <li key={index} className="p-2 rounded share-tech-mono-regular text-xl text-center">
            {formatDate(conn.date)} — {conn.earliest} TO {conn.latest}
          </li>
        ))}
      </ul>
    </div>
  );
};

export default App;