export const configs: {
  placeholder: string;
  value:
    | "name"
    | "username"
    | "host"
    | "port"
    | "default_database"
    | "password";
  type: string;
}[] = [
  {
    placeholder: "Nome do servidor",
    value: "name",
    type: "text",
  },
  {
    placeholder: "Usuário",
    value: "username",
    type: "text",
  },
  {
    placeholder: "Host",
    value: "host",
    type: "text",
  },
  {
    placeholder: "Porta",
    value: "port",
    type: "number",
  },
  {
    placeholder: "Banco de dados padrão",
    value: "default_database",
    type: "text",
  },
  {
    placeholder: "Senha",
    value: "password",
    type: "password",
  },
];
