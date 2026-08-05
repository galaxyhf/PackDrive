import { useMemo, useState } from "react";
import {
  CheckCircle2,
  ChevronRight,
  Database,
  ExternalLink,
  FileArchive,
  Search,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { collectionTutorials } from "./tutorials";

export function TutorialsPage() {
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState(collectionTutorials[0].id);

  const filteredTutorials = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase("pt-BR");
    if (!normalizedQuery) return collectionTutorials;

    return collectionTutorials.filter((tutorial) =>
      [tutorial.system, ...tutorial.required]
        .join(" ")
        .toLocaleLowerCase("pt-BR")
        .includes(normalizedQuery),
    );
  }, [query]);

  const selectedTutorial =
    filteredTutorials.find((tutorial) => tutorial.id === selectedId) ??
    filteredTutorials[0];

  return (
    <div className="tutorials-layout">
      <aside className="tutorial-systems" aria-label="Sistemas disponíveis">
        <div className="tutorial-search search-field">
          <Search size={16} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Buscar sistema ou arquivo"
            aria-label="Buscar tutorial de coleta"
          />
        </div>
        <span className="tutorial-count">
          {filteredTutorials.length} {filteredTutorials.length === 1 ? "sistema" : "sistemas"}
        </span>
        <nav>
          {filteredTutorials.map((tutorial) => (
            <button
              key={tutorial.id}
              className={selectedTutorial?.id === tutorial.id ? "active" : ""}
              aria-current={selectedTutorial?.id === tutorial.id ? "page" : undefined}
              onClick={() => setSelectedId(tutorial.id)}
            >
              <Database size={16} />
              <span>{tutorial.system}</span>
              <ChevronRight size={15} />
            </button>
          ))}
        </nav>
      </aside>

      <section className="tutorial-detail" aria-live="polite">
        {selectedTutorial ? (
          <>
            <header className="tutorial-detail-header">
              <div className="tutorial-system-icon">
                <Database size={20} />
              </div>
              <div>
                <span>Tutorial de coleta</span>
                <h2>{selectedTutorial.system}</h2>
              </div>
            </header>

            <div className="tutorial-detail-body">
              <section className="collection-requirements">
                <div>
                  <FileArchive size={18} />
                  <h3>O que precisa coletar</h3>
                </div>
                <ul>
                  {selectedTutorial.required.map((item) => (
                    <li key={item}>
                      <CheckCircle2 size={15} />
                      <span>{item}</span>
                    </li>
                  ))}
                </ul>
              </section>

              {selectedTutorial.sections.map((section) => (
                <section className="tutorial-section" key={section.title}>
                  <h3>{section.title}</h3>
                  {section.paragraphs?.map((paragraph) => (
                    <p key={paragraph}>{paragraph}</p>
                  ))}
                  {section.steps && (
                    <ol className="tutorial-steps">
                      {section.steps.map((step) => (
                        <li key={step}>
                          <span>{step}</span>
                        </li>
                      ))}
                    </ol>
                  )}
                  {section.fields && (
                    <dl className="tutorial-fields">
                      {section.fields.map((field) => (
                        <div key={field.label}>
                          <dt>{field.label}</dt>
                          <dd>{field.value}</dd>
                        </div>
                      ))}
                    </dl>
                  )}
                  {section.bullets && (
                    <ul className="tutorial-notes">
                      {section.bullets.map((item) => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                  )}
                  {section.link && (
                    <button
                      className="tutorial-external"
                      onClick={() => void openUrl(section.link!.href)}
                    >
                      {section.link.label}
                      <ExternalLink size={14} />
                    </button>
                  )}
                </section>
              ))}
            </div>
          </>
        ) : (
          <div className="tutorial-empty">
            <Search size={24} />
            <strong>Nenhum sistema encontrado</strong>
            <span>Tente buscar por outro nome ou arquivo.</span>
          </div>
        )}
      </section>
    </div>
  );
}
