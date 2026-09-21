cd /home/sosi/soar-agent
sudo deploy/install.sh     # compila, instala y (re)activa el servicio

sudo systemctl start soar-agent    : ya instalado

sudo systemctl enable soar-agent    : volver activarlo despues de PARAR desde el dashboard


COMANDOS TECNICOS 

# Arrancar / parar / ver el programa
sudo systemctl start soar-agent      # arrancar ahora
sudo systemctl stop soar-agent       # parar ahora
sudo systemctl restart soar-agent    # reiniciar
sudo systemctl status soar-agent     # ver si esta vivo
journalctl -u soar-agent -f          # ver lo que pasa en vivo (Ctrl-C para salir)

# Panel web
bash scripts/panel.sh status         # esta encendido o apagado?
bash scripts/panel.sh off            # apagar SOLO el panel (sigue bloqueando)
bash scripts/panel.sh on             # volver a encenderlo
# Abrir en el navegador:  http://127.0.0.1:8787

# Token (llave del panel)
bash scripts/token.sh show           # ver la llave actual
bash scripts/token.sh generate       # crear/rotar una nueva
bash scripts/token.sh clear          # quitarla (panel sin contrasena)

# Que arranque solo al encender el PC (o no)
sudo systemctl enable soar-agent     # SI arrancar al encender
sudo systemctl disable soar-agent    # NO arrancar al encender

# Instalar / reinstalar
sudo deploy/install.sh               # instala y activa al arranque
sudo deploy/install.sh --no-enable   # instala y arranca, sin autostart
