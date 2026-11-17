-- Um schema por banco do BIRD. O dump do Mini-Dev põe as 75 tabelas no
-- `public`; aqui cada uma vai para o schema do seu conjunto de dados, e o
-- db.sh copia cada schema para um database próprio (o `dataset` da pergunta).
-- Gerado do dev_tables.json do BIRD. Pode rodar de novo: o que já foi
-- movido é pulado.

CREATE SCHEMA IF NOT EXISTS california_schools;
ALTER TABLE IF EXISTS public."frpm" SET SCHEMA california_schools;
ALTER TABLE IF EXISTS public."satscores" SET SCHEMA california_schools;
ALTER TABLE IF EXISTS public."schools" SET SCHEMA california_schools;

CREATE SCHEMA IF NOT EXISTS card_games;
ALTER TABLE IF EXISTS public."cards" SET SCHEMA card_games;
ALTER TABLE IF EXISTS public."foreign_data" SET SCHEMA card_games;
ALTER TABLE IF EXISTS public."legalities" SET SCHEMA card_games;
ALTER TABLE IF EXISTS public."sets" SET SCHEMA card_games;
ALTER TABLE IF EXISTS public."set_translations" SET SCHEMA card_games;
ALTER TABLE IF EXISTS public."rulings" SET SCHEMA card_games;

CREATE SCHEMA IF NOT EXISTS codebase_community;
ALTER TABLE IF EXISTS public."badges" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."comments" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."posthistory" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."postlinks" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."posts" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."tags" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."users" SET SCHEMA codebase_community;
ALTER TABLE IF EXISTS public."votes" SET SCHEMA codebase_community;

CREATE SCHEMA IF NOT EXISTS debit_card_specializing;
ALTER TABLE IF EXISTS public."customers" SET SCHEMA debit_card_specializing;
ALTER TABLE IF EXISTS public."gasstations" SET SCHEMA debit_card_specializing;
ALTER TABLE IF EXISTS public."products" SET SCHEMA debit_card_specializing;
ALTER TABLE IF EXISTS public."transactions_1k" SET SCHEMA debit_card_specializing;
ALTER TABLE IF EXISTS public."yearmonth" SET SCHEMA debit_card_specializing;

CREATE SCHEMA IF NOT EXISTS european_football_2;
ALTER TABLE IF EXISTS public."player_attributes" SET SCHEMA european_football_2;
ALTER TABLE IF EXISTS public."player" SET SCHEMA european_football_2;
ALTER TABLE IF EXISTS public."league" SET SCHEMA european_football_2;
ALTER TABLE IF EXISTS public."country" SET SCHEMA european_football_2;
ALTER TABLE IF EXISTS public."team" SET SCHEMA european_football_2;
ALTER TABLE IF EXISTS public."team_attributes" SET SCHEMA european_football_2;
ALTER TABLE IF EXISTS public."match" SET SCHEMA european_football_2;

CREATE SCHEMA IF NOT EXISTS financial;
ALTER TABLE IF EXISTS public."account" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."card" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."client" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."disp" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."district" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."loan" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."order" SET SCHEMA financial;
ALTER TABLE IF EXISTS public."trans" SET SCHEMA financial;

CREATE SCHEMA IF NOT EXISTS formula_1;
ALTER TABLE IF EXISTS public."circuits" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."constructors" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."drivers" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."seasons" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."races" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."constructorresults" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."constructorstandings" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."driverstandings" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."laptimes" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."pitstops" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."qualifying" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."status" SET SCHEMA formula_1;
ALTER TABLE IF EXISTS public."results" SET SCHEMA formula_1;

CREATE SCHEMA IF NOT EXISTS student_club;
ALTER TABLE IF EXISTS public."event" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."major" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."zip_code" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."attendance" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."budget" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."expense" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."income" SET SCHEMA student_club;
ALTER TABLE IF EXISTS public."member" SET SCHEMA student_club;

CREATE SCHEMA IF NOT EXISTS superhero;
ALTER TABLE IF EXISTS public."alignment" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."attribute" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."colour" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."gender" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."publisher" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."race" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."superhero" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."hero_attribute" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."superpower" SET SCHEMA superhero;
ALTER TABLE IF EXISTS public."hero_power" SET SCHEMA superhero;

CREATE SCHEMA IF NOT EXISTS thrombosis_prediction;
ALTER TABLE IF EXISTS public."examination" SET SCHEMA thrombosis_prediction;
ALTER TABLE IF EXISTS public."patient" SET SCHEMA thrombosis_prediction;
ALTER TABLE IF EXISTS public."laboratory" SET SCHEMA thrombosis_prediction;

CREATE SCHEMA IF NOT EXISTS toxicology;
ALTER TABLE IF EXISTS public."atom" SET SCHEMA toxicology;
ALTER TABLE IF EXISTS public."bond" SET SCHEMA toxicology;
ALTER TABLE IF EXISTS public."connected" SET SCHEMA toxicology;
ALTER TABLE IF EXISTS public."molecule" SET SCHEMA toxicology;
