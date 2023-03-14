const messagesDiv = document.getElementById('messages');

const scrollToBottom = () => {
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
};

const newMessage = document.createElement('div');
newMessage.classList.add('msg-wrapper');
messagesDiv.appendChild(newMessage);
scrollToBottom();
