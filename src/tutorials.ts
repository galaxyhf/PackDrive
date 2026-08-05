export interface CollectionTutorialSection {
  title: string;
  paragraphs?: string[];
  bullets?: string[];
  steps?: string[];
  fields?: Array<{ label: string; value: string }>;
  link?: { label: string; href: string };
}

export interface CollectionTutorial {
  id: string;
  system: string;
  required: string[];
  sections: CollectionTutorialSection[];
}

export const collectionTutorials: CollectionTutorial[] = [
  {
    id: "dominio",
    system: "Domínio",
    required: ["CONTABIL.DB", "CONTABIL.LOG"],
    sections: [
      {
        title: "Procedimento inicial antes do backup",
        steps: [
          "Acesse o sistema Domínio.",
          "Vá até Controle > Permissões > Usuários.",
          "Na tela de usuários, clique em Novo.",
          "Preencha o cadastro com as informações abaixo.",
          "Clique em Gravar.",
          "Para conferir se o usuário foi criado, acesse a aba Listagem e clique em Buscar.",
        ],
        fields: [
          { label: "Nome", value: "CONVERSOR" },
          { label: "Usuário", value: "CONVERSOR" },
          { label: "Senha", value: "1" },
          { label: "Confirmar senha", value: "1" },
          { label: "Tipo de acesso", value: "Externo" },
          { label: "Situação", value: "Ativo" },
        ],
      },
      {
        title: "Backup local",
        steps: [
          "Dentro do sistema Domínio, acesse Utilitários.",
          "Clique na opção Backup.",
          "Quando aparecer a mensagem de confirmação, clique em OK.",
          "Para verificar o diretório onde o backup será salvo, acesse Utilitários > Configurar Backup.",
        ],
      },
      {
        title: "Backup na nuvem Onvio (Feito automaticamente, não é possível realizar de forma manual)",
        steps: [
          "Acesse a área de Suporte da Onvio.",
          "Entre em Backup.",
          "Acesse a opção Download.",
          "Localize e baixe o backup disponível.",
        ],
      },
    ],
  },
  {
    id: "mastermaq",
    system: "Mastermaq",
    required: ["NG", "NG_FOLHA", "NG_CONTABIL", "NG_RH", "NG_DOMINIO", "NG_PONTO"],
    sections: [
      {
        title: "Antes de começar",
        paragraphs: [
          "Esses arquivos ficam dentro da pasta do sistema, mas também são incluídos no backup completo. O procedimento recomendado é gerar o backup completo e deixar na pasta da coleta apenas os arquivos informados acima.",
        ],
      },
      {
        title: "Realizando o backup",
        steps: [
          "No menu Iniciar do Windows, pesquise por NG Tools ou Toolbox.",
          "Abra a ferramenta.",
          "Clique na opção Backup Completo.",
          "Inicie o backup.",
          "O arquivo será salvo no diretório selecionado.",
        ],
      },
    ],
  },
  {
    id: "contmatic-postgres",
    system: "Contmatic Postgres",
    required: ["Backup completo do PostgreSQL"],
    sections: [],
  },
  {
    id: "dexion-firebird",
    system: "Dexion Firebird",
    required: ["dexion.fdb"],
    sections: [],
  },
  {
    id: "fortes",
    system: "Fortes",
    required: ["AC.FDB"],
    sections: [],
  },
  {
    id: "fortes-sql",
    system: "Fortes SQL",
    required: ["Backup completo do sistema na extensão .bak"],
    sections: [],
  },
  {
    id: "esocial",
    system: "eSocial",
    required: ["XMLs de todos os eventos enviados desde o início da obrigatoriedade"],
    sections: [
      {
        title: "Observações importantes",
        bullets: [
          "Selecione períodos pequenos para consulta, de no máximo uma semana.",
          "Ao baixar períodos maiores, o Portal do eSocial pode não entregar todos os XMLs.",
          "Caso faltem arquivos, a conversão poderá ficar sem informações.",
          "O nome dos arquivos deve conter o evento do eSocial.",
          "Colete todos os XMLs desde o primeiro envio da empresa para importar todos os cadastros e movimentos enviados.",
        ],
        link: {
          label: "Consultar procedimento oficial no Governo Federal",
          href: "https://www.gov.br/esocial/pt-br/noticias/esocial-download-para-facilitar-a-vida-do-empregador",
        },
      },
    ],
  },
  {
    id: "nasajon-postgresql",
    system: "Nasajon PostgreSQL",
    required: ["Backup geral gerado pelo sistema ou backup do PostgreSQL"],
    sections: [
      {
        title: "Realizando o backup",
        steps: [
          "Abra o sistema principal do Nasajon.",
          "Clique no ícone Admin.",
          "Acesse a aba Ferramentas.",
          "Abra o módulo Backup/Restore.",
          "Clique em Próximo.",
          "Na tela de tipo de serviço, selecione Nova Cópia de Segurança.",
          "Avance para a próxima etapa.",
          "Selecione a base de dados desejada, caso o cliente tenha mais de uma.",
          "Informe o diretório onde o arquivo será gerado.",
          "Conclua o processo de backup.",
        ],
      },
    ],
  },
  {
    id: "questor",
    system: "Questor",
    required: ["QUESTOR.FDB"],
    sections: [],
  },
  {
    id: "sci-visual",
    system: "SCI Visual",
    required: ["Pasta VPRA", "Pasta VSUC"],
    sections: [],
  },
  {
    id: "sped-contabil",
    system: "SPED Contábil",
    required: ["SPED.txt"],
    sections: [],
  },
  {
    id: "totvs-protheus",
    system: "TOTVS Protheus",
    required: ["Backup .BAK do SQL Server", "SIGAMAT.EMP em formato DBF"],
    sections: [
      {
        title: "Quando o SIGAMAT.EMP estiver em CTREE",
        paragraphs: [
          "O arquivo SIGAMAT.EMP deve estar no formato DBF. Se ele não abrir em um gerenciador de banco de dados DBF, faça a conversão abaixo.",
        ],
        steps: [
          "Abra o aplicativo SmartClient.",
          "Abra o aplicativo APSDU.",
          "Solicite que o cliente entre com um usuário administrador do sistema.",
          "Acesse Arquivo > Abrir.",
          "Selecione o driver utilizado pelo cliente, como CTREE.",
          "Procure pelo arquivo SIGAMAT.EMP.",
          "Na caixa de pesquisa, selecione Todos os Arquivos.",
          "Abra o arquivo SIGAMAT.EMP.",
          "Acesse Utilitário > Copiar para.",
          "Selecione o driver DBF.",
          "Clique em OK.",
          "Aguarde a finalização da cópia.",
        ],
      },
      {
        title: "Arquivo gerado",
        paragraphs: [
          "O arquivo será criado em Protheus_Data\\System com o nome SIGAMAT1.DBF. Copie esse arquivo e utilize-o na conversão junto com o backup .BAK do SQL Server.",
        ],
      },
    ],
  },
  {
    id: "totvs-rm",
    system: "TOTVS RM",
    required: ["Base em SQL Server"],
    sections: [],
  },
  {
    id: "sci-unico",
    system: "SCI Único",
    required: ["VSCI.SDB"],
    sections: [],
  },
];
